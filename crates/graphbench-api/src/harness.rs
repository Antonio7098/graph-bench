use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use graphbench_core::{
    fixtures::FixtureRepository, load_task_spec, GraphWorkspace, RepresentationLevel,
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
use tracing::{info, warn};

use crate::event_stream::{now_rfc3339, EventStream, StreamEvent};

#[derive(Debug)]
pub struct BenchmarkConfig {
    pub run_id: String,
    pub task_spec_path: String,
    pub fixture_path: String,
    pub model_id: Option<String>,
    pub api_key: Option<String>,
    pub strategy: String,
    pub turn_budget: u32,
    pub timeout_ms: u64,
    pub token_budget: u32,
    pub prompt_headroom: u32,
    pub seed_overview: u32,
    pub initial_select: String,
    pub representation_level: String,
}

pub async fn run_benchmark(
    config: BenchmarkConfig,
    event_stream: Arc<EventStream>,
) -> Result<(String, String)> {
    tokio::task::spawn_blocking(move || run_benchmark_sync(config, event_stream))
        .await
        .context("Benchmark worker task panicked")?
}

fn run_benchmark_sync(
    config: BenchmarkConfig,
    event_stream: Arc<EventStream>,
) -> Result<(String, String)> {
    let run_id = config.run_id.clone();

    info!("[{}] Starting benchmark with config: {:?}", run_id, config);
    publish_system_event(
        &event_stream,
        &run_id,
        "harness",
        "run.started",
        "info",
        "Starting benchmark",
        None,
        None,
        None,
        json!({
            "task_spec_path": config.task_spec_path,
            "fixture_path": config.fixture_path,
            "strategy": config.strategy,
        }),
    );

    let api_key = config
        .api_key
        .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
        .context("OPENROUTER_API_KEY not provided - set in request or environment")?;
    let model = config
        .model_id
        .or_else(|| std::env::var("OPENROUTER_MODEL_ID").ok())
        .unwrap_or_else(|| "nvidia/nemotron-3-nano-30b-a3b:free".to_owned());

    let fixture_repository = FixtureRepository;
    let fixture_path = PathBuf::from(&config.fixture_path);
    let (fixture, resolution) = fixture_repository
        .load(&fixture_path)
        .context(format!("Failed to load fixture from {}", config.fixture_path))?;

    let workspace =
        GraphWorkspace::load(fixture.clone(), &resolution).context("Failed to load GraphWorkspace")?;
    let mut graph_session = workspace.session();
    let events = Arc::new(std::sync::Mutex::new(Vec::<CapturedEvent>::new()));

    graph_session.seed_overview(Some(config.seed_overview as usize));
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: format!("seed_overview(limit={})", config.seed_overview),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );

    let rep_level = match config.representation_level.to_uppercase().as_str() {
        "L0" => RepresentationLevel::L0,
        "L1" => RepresentationLevel::L1,
        "L2" => RepresentationLevel::L2,
        _ => {
            warn!(
                "[{}] Unknown representation level '{}', defaulting to L1",
                run_id, config.representation_level
            );
            RepresentationLevel::L1
        }
    };

    graph_session.select(&config.initial_select, rep_level)?;
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: format!("select({},{})", config.initial_select, config.representation_level),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );

    graph_session.hydrate_exact_proof(&config.initial_select, 8)?;
    record_event(
        &events,
        HarnessEvent::GraphSessionMutated {
            mutation: format!("hydrate_exact_proof({},8)", config.initial_select),
            graph_session_hash: hash_graph_session(&graph_session.session_json()?),
        },
    );

    let live_graph_state =
        LiveGraphState::new(workspace, graph_session, resolution.snapshot_path.clone(), 2200);
    ensure_python_query_runtime_ready().context("Failed to initialize Python query runtime")?;

    let mut tools = ToolRegistry::default();
    live_graph_state.register_tools(&mut tools);

    let task = load_task_spec(&config.task_spec_path)
        .context(format!("Failed to load task spec from {}", config.task_spec_path))?;
    let snapshot = live_graph_state.snapshot()?;
    let strategy = match config.strategy.as_str() {
        "graph_then_targeted_lexical_read" => graph_then_targeted_lexical_read(),
        _ => {
            warn!("[{}] Unknown strategy '{}', using default", run_id, config.strategy);
            graph_then_targeted_lexical_read()
        }
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
            turn_budget: config.turn_budget.max(48),
            timeout_ms: config.timeout_ms,
            token_budget: config.token_budget,
            prompt_headroom: config.prompt_headroom,
            prompt_version: "v1".to_owned(),
            strategy,
            harness_version: "0.1.0".to_owned(),
        },
    };

    let mut model_client =
        OpenRouterClient::new(api_key, model).context("Failed to create OpenRouter client")?;

    let harness_events = Arc::clone(&events);
    let graph_state_provider = live_graph_state.clone();
    let event_stream_clone = Arc::clone(&event_stream);
    let run_id_clone = run_id.clone();

    let mut runner = HarnessRunner::new(&mut model_client, &tools)
        .with_graph_state_provider(move || graph_state_provider.snapshot())
        .with_telemetry_hook(move |event| {
            record_event(&harness_events, event.clone());
            publish_harness_event(&event_stream_clone, &run_id_clone, event);
        });

    let output = runner.execute(&input).context("Harness execution failed")?;

    let trace_path = PathBuf::from(format!("traces/{}.json", input.run_id));
    output
        .turn_ledger
        .save(&trace_path)
        .context("Failed to save turn ledger")?;
    output
        .turn_ledger
        .replay_validate()
        .context("Turn ledger replay validation failed")?;

    let blob_store = BlobStore::new("traces/blobs").context("Failed to create blob store")?;
    let recorded_invocations = output
        .model_invocations
        .iter()
        .map(RecordedModelInvocation::from)
        .collect::<Vec<_>>();

    let event_snapshot = events.lock().expect("captured events lock").clone();
    let bundle = build_observability_bundle(
        &input,
        &output,
        &recorded_invocations,
        &event_snapshot,
        &blob_store,
    )
    .context("Failed to build observability bundle")?;

    let observability_path = PathBuf::from(format!("traces/{}.observability.json", input.run_id));
    bundle
        .save(&observability_path)
        .context("Failed to save observability bundle")?;
    bundle.validate(".").context("Observability validation failed")?;

    let structured_logs_path = PathBuf::from(format!("traces/{}.events.jsonl", input.run_id));
    bundle
        .save_structured_logs_jsonl(&structured_logs_path)
        .context("Failed to save structured logs")?;

    let final_message = format!(
        "run_id={}\ntrace={}\nobservability={}\nstructured_logs={}\nfinal_state={:?}\nfinal_message={}",
        input.run_id,
        trace_path.display(),
        observability_path.display(),
        structured_logs_path.display(),
        output.final_state,
        output.final_message
    );

    Ok((run_id, final_message))
}

fn record_event(
    events: &Arc<std::sync::Mutex<Vec<CapturedEvent>>>,
    event: HarnessEvent,
) {
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

fn publish_system_event(
    event_stream: &EventStream,
    run_id: &str,
    component: &str,
    event_type: &str,
    level: &str,
    message: &str,
    turn_index: Option<u32>,
    tool_name: Option<String>,
    metrics: Option<Value>,
    details: Value,
) {
    event_stream.publish(StreamEvent {
        seq: 0,
        captured_at: now_rfc3339(),
        stream: "live".to_owned(),
        run_id: Some(run_id.to_owned()),
        component: component.to_owned(),
        event_type: event_type.to_owned(),
        level: level.to_owned(),
        message: message.to_owned(),
        turn_index,
        tool_name,
        provider_request_id: None,
        metrics,
        tags: vec![component.to_owned()],
        details,
    });
}

fn publish_harness_event(event_stream: &EventStream, run_id: &str, event: &HarnessEvent) {
    let details = serde_json::to_value(event).unwrap_or_else(|_| json!({}));
    match event {
        HarnessEvent::RunStarted { .. } => publish_system_event(
            event_stream,
            run_id,
            "harness",
            "run.started",
            "info",
            "Run started",
            None,
            None,
            None,
            details,
        ),
        HarnessEvent::TurnStarted { turn_index, .. } => publish_system_event(
            event_stream,
            run_id,
            "harness",
            "turn.started",
            "info",
            &format!("Turn {} started", turn_index),
            Some(*turn_index),
            None,
            None,
            details,
        ),
        HarnessEvent::PromptAssembled { turn_index, .. } => publish_system_event(
            event_stream,
            run_id,
            "context",
            "prompt.assembled",
            "info",
            &format!("Turn {} prompt assembled", turn_index),
            Some(*turn_index),
            None,
            None,
            details,
        ),
        HarnessEvent::ModelRequestSent { turn_index } => publish_system_event(
            event_stream,
            run_id,
            "provider",
            "model.request_sent",
            "info",
            &format!("Turn {} model request sent", turn_index),
            Some(*turn_index),
            None,
            None,
            details,
        ),
        HarnessEvent::ModelResponseReceived {
            turn_index,
            provider_request_id,
        } => event_stream.publish(StreamEvent {
            seq: 0,
            captured_at: now_rfc3339(),
            stream: "live".to_owned(),
            run_id: Some(run_id.to_owned()),
            component: "provider".to_owned(),
            event_type: "model.response_received".to_owned(),
            level: "info".to_owned(),
            message: format!("Turn {} model response received", turn_index),
            turn_index: Some(*turn_index),
            tool_name: None,
            provider_request_id: provider_request_id.clone(),
            metrics: None,
            tags: vec!["provider".to_owned()],
            details,
        }),
        HarnessEvent::ModelResponseRejected { turn_index, error } => publish_system_event(
            event_stream,
            run_id,
            "provider",
            "model.response_rejected",
            "error",
            &format!("Turn {} model response rejected: {}", turn_index, error),
            Some(*turn_index),
            None,
            None,
            details,
        ),
        HarnessEvent::ModelResponseValidated { turn_index, .. } => publish_system_event(
            event_stream,
            run_id,
            "provider",
            "model.response_validated",
            "info",
            &format!("Turn {} model response validated", turn_index),
            Some(*turn_index),
            None,
            None,
            details,
        ),
        HarnessEvent::ToolRequested {
            turn_index,
            tool_name,
        } => publish_system_event(
            event_stream,
            run_id,
            "tool",
            "tool.requested",
            "info",
            &format!("Turn {} requested {}", turn_index, tool_name),
            Some(*turn_index),
            Some(tool_name.clone()),
            None,
            details,
        ),
        HarnessEvent::ToolStarted {
            turn_index,
            tool_name,
        } => publish_system_event(
            event_stream,
            run_id,
            "tool",
            "tool.started",
            "info",
            &format!("Turn {} started {}", turn_index, tool_name),
            Some(*turn_index),
            Some(tool_name.clone()),
            None,
            details,
        ),
        HarnessEvent::ToolCompleted {
            turn_index,
            tool_name,
        } => publish_system_event(
            event_stream,
            run_id,
            "tool",
            "tool.completed",
            "info",
            &format!("Turn {} completed {}", turn_index, tool_name),
            Some(*turn_index),
            Some(tool_name.clone()),
            None,
            details,
        ),
        HarnessEvent::ToolFailed {
            turn_index,
            tool_name,
            error,
        } => publish_system_event(
            event_stream,
            run_id,
            "tool",
            "tool.failed",
            "error",
            &format!("Turn {} failed {}: {}", turn_index, tool_name, error),
            Some(*turn_index),
            Some(tool_name.clone()),
            None,
            details,
        ),
        HarnessEvent::ReadinessChanged {
            turn_index,
            readiness_state,
        } => publish_system_event(
            event_stream,
            run_id,
            "harness",
            "readiness.changed",
            "info",
            &format!("Turn {} readiness changed to {:?}", turn_index, readiness_state),
            Some(*turn_index),
            None,
            None,
            details,
        ),
        HarnessEvent::GraphSessionMutated { .. } => publish_system_event(
            event_stream,
            run_id,
            "graph",
            "graph.session_mutated",
            "info",
            "Graph session mutated",
            None,
            None,
            None,
            details,
        ),
        HarnessEvent::EvidenceMatched { turn_index, .. } => publish_system_event(
            event_stream,
            run_id,
            "harness",
            "evidence.matched",
            "info",
            &format!("Turn {} matched evidence", turn_index),
            Some(*turn_index),
            None,
            None,
            details,
        ),
        HarnessEvent::RunFailed { error, .. } => publish_system_event(
            event_stream,
            run_id,
            "harness",
            "run.failed",
            "error",
            error,
            None,
            None,
            None,
            details,
        ),
        HarnessEvent::RunCompleted { turns, .. } => publish_system_event(
            event_stream,
            run_id,
            "harness",
            "run.completed",
            "info",
            &format!("Run completed in {} turns", turns),
            None,
            None,
            Some(json!({ "turns": turns })),
            details,
        ),
    }
}
