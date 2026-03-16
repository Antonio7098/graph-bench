use clap::Parser;
use graphbench_core::{
    fixtures::FixtureRepository, load_task_spec, AppError, ErrorCode, ErrorContext,
    GraphWorkspace, RepresentationLevel,
};
use graphbench_harness::{
    ensure_python_query_runtime_ready, graph_then_targeted_lexical_read,
    graph_tools::LiveGraphState,
    observability::{
        build_observability_bundle, BlobStore, CapturedEvent, RecordedModelInvocation,
    },
    openrouter::OpenRouterClient,
    HarnessEvent, HarnessInput, HarnessRunConfig, HarnessRunner, ObjectiveState, ToolRegistry,
};
use sha2::Digest;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(name = "smoke_openrouter")]
#[command(about = "Run a configurable benchmark with OpenRouter", long_about = None)]
struct Args {
    #[arg(long, default_value = "tasks/prepare-to-edit/task-01.task.json")]
    task_spec: PathBuf,
    #[arg(long, default_value = "fixtures/graphbench-internal/fixture.json")]
    fixture: PathBuf,
    #[arg(long, default_value = "graph_then_targeted_lexical_read")]
    strategy: String,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, default_value_t = 48)]
    turn_budget: u32,
    #[arg(long, default_value_t = 300_000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 2_000_000)]
    token_budget: u32,
    #[arg(long, default_value_t = 24576)]
    prompt_headroom: u32,
    #[arg(long)]
    run_id: Option<String>,
    #[arg(long)]
    api_key: Option<String>,
    #[arg(long, default_value_t = 2)]
    seed_overview: u32,
    #[arg(long, default_value = "crates/graphbench-core/src/artifacts.rs")]
    initial_select: String,
    #[arg(long, default_value = "L1")]
    representation_level: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let args = Args::parse();

    let dotenv = load_dotenv()?;
    let api_key = args
        .api_key
        .or_else(|| env::var("OPENROUTER_API_KEY").ok())
        .or_else(|| dotenv.get("OPENROUTER_API_KEY").cloned())
        .ok_or_else(|| missing_env("OPENROUTER_API_KEY"))?;

    let model = args
        .model
        .or_else(|| env::var("OPENROUTER_MODEL_ID").ok())
        .or_else(|| dotenv.get("OPENROUTER_MODEL_ID").cloned())
        .unwrap_or_else(|| "nvidia/nemotron-3-nano-30b-a3b:free".to_owned());

    let fixture_repository = FixtureRepository;
    let (fixture, resolution) = fixture_repository.load(&args.fixture)?;
    let workspace = GraphWorkspace::load(fixture.clone(), &resolution)?;
    let mut graph_session = workspace.session();
    let events = Arc::new(Mutex::new(Vec::<CapturedEvent>::new()));

    graph_session.seed_overview(Some(args.seed_overview as usize));
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: format!("seed_overview(limit={})", args.seed_overview),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );

    let rep_level = match args.representation_level.to_uppercase().as_str() {
        "L0" => RepresentationLevel::L0,
        "L1" => RepresentationLevel::L1,
        "L2" => RepresentationLevel::L2,
        _ => RepresentationLevel::L1,
    };

    graph_session.select(&args.initial_select, rep_level)?;
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: format!(
                "select({},{})",
                args.initial_select, args.representation_level
            ),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );

    graph_session.hydrate_exact_proof(&args.initial_select, 8)?;
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: format!("hydrate_exact_proof({},8)", args.initial_select),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );

    let live_graph_state = LiveGraphState::new(
        workspace,
        graph_session,
        resolution.snapshot_path.clone(),
        2200,
    );
    ensure_python_query_runtime_ready()?;

    let mut tools = ToolRegistry::default();
    live_graph_state.register_tools(&mut tools);

    let task = load_task_spec(&args.task_spec)?;
    let snapshot = live_graph_state.snapshot()?;

    let run_id = args.run_id.unwrap_or_else(|| {
        format!(
            "smoke-openrouter-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_secs()
        )
    });

    let strategy = match args.strategy.as_str() {
        "graph_then_targeted_lexical_read" => graph_then_targeted_lexical_read(),
        _ => graph_then_targeted_lexical_read(),
    };

    let input = HarnessInput {
        run_id: run_id.clone(),
        fixture_id: fixture.fixture_id,
        task_id: task.task_id.clone(),
        objective: ObjectiveState {
            task_statement: task.statement,
            task_class: "prepare_to_edit".to_owned(),
            allowed_tools: tools
                .contracts()
                .iter()
                .map(|contract| format!("{}@{}", contract.name, contract.version))
                .collect(),
            verification_targets: task
                .verification_targets
                .iter()
                .map(|target| format!("{}: {}", target.kind, target.value))
                .collect(),
            unresolved_questions: Vec::new(),
        },
        graph_prompt: snapshot.graph_prompt,
        graph_session_snapshot: snapshot.graph_session_snapshot,
        config: HarnessRunConfig {
            turn_budget: args.turn_budget.max(48),
            timeout_ms: args.timeout_ms,
            token_budget: args.token_budget,
            prompt_headroom: args.prompt_headroom,
            prompt_version: "v1".to_owned(),
            strategy,
            harness_version: "0.1.0".to_owned(),
        },
    };

    let mut model_client = OpenRouterClient::new(api_key, model)?;
    let harness_events = Arc::clone(&events);
    let graph_state_provider = live_graph_state.clone();
    let mut runner = HarnessRunner::new(&mut model_client, &tools)
        .with_graph_state_provider(move || graph_state_provider.snapshot())
        .with_telemetry_hook(move |event| {
            record_event(&harness_events, event.clone());
        });
    let output = runner.execute(&input)?;

    let path = PathBuf::from(format!("traces/{}.json", input.run_id));
    output.turn_ledger.save(&path)?;
    output.turn_ledger.replay_validate()?;

    let blob_store = BlobStore::new("traces/blobs")?;
    let recorded_invocations = output
        .model_invocations
        .iter()
        .map(RecordedModelInvocation::from)
        .collect::<Vec<_>>();
    let event_snapshot = events
        .lock()
        .expect("event capture lock should not be poisoned")
        .clone();
    let bundle = build_observability_bundle(
        &input,
        &output,
        &recorded_invocations,
        &event_snapshot,
        &blob_store,
    )?;
    let observability_path = PathBuf::from(format!("traces/{}.observability.json", input.run_id));
    bundle.save(&observability_path)?;
    bundle.validate(".")?;
    let structured_logs_path = PathBuf::from(format!("traces/{}.events.jsonl", input.run_id));
    bundle.save_structured_logs_jsonl(&structured_logs_path)?;

    println!("run_id={}", input.run_id);
    println!("trace={}", path.display());
    println!("observability={}", observability_path.display());
    println!("structured_logs={}", structured_logs_path.display());
    println!("final_state={:?}", output.final_state);
    println!("final_message={}", output.final_message);
    Ok(())
}

fn record_event(events: &Arc<Mutex<Vec<CapturedEvent>>>, event: HarnessEvent) {
    if let Ok(mut captured) = events.lock() {
        captured.push(CapturedEvent {
            captured_at: chrono::Utc::now().to_rfc3339(),
            event,
        });
    }
}

fn hash_graph_session(snapshot: &str) -> String {
    format!("sha256:{:x}", sha2::Sha256::digest(snapshot.as_bytes()))
}

fn load_dotenv() -> Result<BTreeMap<String, String>, AppError> {
    let path = PathBuf::from(".env");
    let contents = fs::read_to_string(&path).map_err(|source| {
        AppError::with_source(
            ErrorCode::ConfigurationInvalid,
            format!("failed to read {}", path.display()),
            ErrorContext {
                component: "smoke_openrouter",
                operation: "read_dotenv",
            },
            source,
        )
    })?;

    let mut values = BTreeMap::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            values.insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    Ok(values)
}

fn missing_env(key: &str) -> AppError {
    AppError::new(
        ErrorCode::ConfigurationInvalid,
        format!("missing required environment variable {key}"),
        ErrorContext {
            component: "smoke_openrouter",
            operation: "read_env",
        },
    )
}
