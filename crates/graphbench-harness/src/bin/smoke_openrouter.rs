use graphbench_core::{
    GraphWorkspace, RepresentationLevel, fixtures::FixtureRepository, load_task_spec,
};
use graphbench_harness::{
    HarnessEvent, HarnessInput, HarnessRunConfig, HarnessRunner, ObjectiveState, ToolRegistry,
    ensure_python_query_runtime_ready, graph_then_targeted_lexical_read,
    graph_tools::LiveGraphState,
    observability::{
        BlobStore, CapturedEvent, RecordedModelInvocation, build_observability_bundle,
    },
    openrouter::OpenRouterClient,
};
use sha2::Digest;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), graphbench_core::AppError> {
    let dotenv = load_dotenv()?;
    let api_key = read_env("OPENROUTER_API_KEY", &dotenv)?;
    let model = read_env("OPENROUTER_MODEL_ID", &dotenv)?;
    let task_spec_path = env::var("GRAPHBENCH_TASK_SPEC_PATH")
        .ok()
        .unwrap_or_else(|| "tasks/prepare-to-edit/task-01.task.json".to_owned());

    let fixture_repository = FixtureRepository;
    let fixture_path = PathBuf::from("fixtures/graphbench-internal/fixture.json");
    let (fixture, resolution) = fixture_repository.load(&fixture_path)?;
    let workspace = GraphWorkspace::load(fixture.clone(), &resolution)?;
    let mut graph_session = workspace.session();
    let events = Arc::new(Mutex::new(Vec::<CapturedEvent>::new()));
    graph_session.seed_overview(Some(2));
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: "seed_overview(limit=2)".to_owned(),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );
    graph_session.select(
        "crates/graphbench-core/src/artifacts.rs",
        RepresentationLevel::L1,
    )?;
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: "select(crates/graphbench-core/src/artifacts.rs,L1)".to_owned(),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );
    graph_session.hydrate_exact_proof("crates/graphbench-core/src/artifacts.rs", 8)?;
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: "hydrate_exact_proof(crates/graphbench-core/src/artifacts.rs,8)".to_owned(),
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

    let task = load_task_spec(&task_spec_path)?;
    let snapshot = live_graph_state.snapshot()?;
    let input = HarnessInput {
        run_id: format!(
            "smoke-openrouter-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_secs()
        ),
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
            turn_budget: task.turn_budget.max(48),
            timeout_ms: 300_000,
            token_budget: 2_000_000,
            prompt_headroom: 24_576,
            prompt_version: "v1".to_owned(),
            strategy: graph_then_targeted_lexical_read(),
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

fn load_dotenv() -> Result<BTreeMap<String, String>, graphbench_core::AppError> {
    let path = PathBuf::from(".env");
    let contents = fs::read_to_string(&path).map_err(|source| {
        graphbench_core::AppError::with_source(
            graphbench_core::ErrorCode::ConfigurationInvalid,
            format!("failed to read {}", path.display()),
            graphbench_core::ErrorContext {
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

fn read_env(
    key: &'static str,
    dotenv: &BTreeMap<String, String>,
) -> Result<String, graphbench_core::AppError> {
    env::var(key)
        .ok()
        .or_else(|| dotenv.get(key).cloned())
        .ok_or_else(|| missing_env(key))
}

fn missing_env(key: &'static str) -> graphbench_core::AppError {
    graphbench_core::AppError::new(
        graphbench_core::ErrorCode::ConfigurationInvalid,
        format!("missing required environment variable {key}"),
        graphbench_core::ErrorContext {
            component: "smoke_openrouter",
            operation: "read_env",
        },
    )
}
