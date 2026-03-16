use graphbench_core::{
    GraphWorkspace, ReadinessState, RepresentationLevel, fixtures::FixtureRepository,
    load_task_spec,
};
use graphbench_harness::{
    HarnessEvent, HarnessInput, HarnessModelResponse, HarnessRunConfig, HarnessRunner, ModelClient,
    ModelInvocation, ModelResponseKind, ObjectiveState, ToolCall, ToolRegistry,
    broad_graph_discovery, ensure_python_query_runtime_ready,
    graph_tools::LiveGraphState,
    observability::{
        BlobStore, CapturedEvent, RecordedModelInvocation, build_observability_bundle,
    },
};
use serde_json::json;
use sha2::Digest;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), graphbench_core::AppError> {
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

    let task = load_task_spec("tasks/prepare-to-edit/task-01.task.json")?;
    let live_graph_state = LiveGraphState::new(
        workspace,
        graph_session,
        resolution.snapshot_path.clone(),
        1600,
    );
    ensure_python_query_runtime_ready()?;
    let snapshot = live_graph_state.snapshot()?;

    let mut tools = ToolRegistry::default();
    live_graph_state.register_tools(&mut tools);

    let input = HarnessInput {
        run_id: format!(
            "smoke-mock-{}",
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
            turn_budget: 3,
            timeout_ms: 180_000,
            token_budget: 100_000,
            prompt_headroom: 512,
            prompt_version: "v1".to_owned(),
            strategy: broad_graph_discovery(),
            harness_version: "0.1.0".to_owned(),
        },
    };

    let mut model = MockModel { calls: 0 };

    let harness_events = Arc::clone(&events);
    let graph_state_provider = live_graph_state.clone();
    let mut runner = HarnessRunner::new(&mut model, &tools)
        .with_graph_state_provider(move || graph_state_provider.snapshot())
        .with_telemetry_hook(move |event| {
            record_event(&harness_events, event.clone());
        });
    let output = runner.execute(&input)?;
    let trace_path = PathBuf::from(format!("traces/{}.json", input.run_id));
    output.turn_ledger.save(&trace_path)?;
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
    println!("trace={}", trace_path.display());
    println!("observability={}", observability_path.display());
    println!("structured_logs={}", structured_logs_path.display());
    println!("final_state={:?}", output.final_state);
    println!("final_message={}", output.final_message);
    Ok(())
}

struct MockModel {
    calls: usize,
}

impl ModelClient for MockModel {
    fn respond(&mut self, _prompt: &str) -> Result<ModelInvocation, graphbench_core::AppError> {
        self.calls += 1;
        let response = if self.calls == 1 {
            HarnessModelResponse {
                kind: ModelResponseKind::ToolCall,
                prompt_version: "v1".to_owned(),
                model_slug: "mock-model".to_owned(),
                provider: "mock-provider".to_owned(),
                assistant_message: "Need graph-backed expansion.".to_owned(),
                tool_call: Some(ToolCall {
                    tool_name: "run_python_query".to_owned(),
                    payload: serde_json::json!({
                        "code": "target = graph.find(path_regex=r'crates/graphbench-core/src/artifacts\\.rs', limit=1)[0]\nsession.walk(target, mode='file', depth=1, limit=6)\nresult = {'target': target['logical_key'], 'selected': session.summary()['selected']}",
                        "include_export": true,
                        "export_kwargs": {
                            "compact": true,
                            "max_frontier_actions": 4,
                            "visible_levels": 3
                        },
                        "limits": {
                            "max_seconds": 2.0,
                            "max_operations": 40,
                            "max_trace_events": 2000,
                            "max_stdout_chars": 2000
                        }
                    }),
                }),
                acquired_fact_ids: vec!["prepare_evidence".to_owned()],
                readiness_state: ReadinessState::EvidenceAcquired,
            }
        } else {
            HarnessModelResponse {
                kind: ModelResponseKind::Complete,
                prompt_version: "v1".to_owned(),
                model_slug: "mock-model".to_owned(),
                provider: "mock-provider".to_owned(),
                assistant_message: "Mock smoke completed.".to_owned(),
                tool_call: None,
                acquired_fact_ids: vec!["schema_constants".to_owned()],
                readiness_state: ReadinessState::ReadyToEdit,
            }
        };

        Ok(ModelInvocation {
            response,
            raw_request: json!({
                "provider": "mock-provider",
                "prompt": "mock",
            }),
            raw_response: json!({
                "mock": true,
                "call_index": self.calls,
            }),
            provider_request_id: Some(format!("mock-request-{}", self.calls)),
        })
    }
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
