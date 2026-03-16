pub mod graph_tools;
pub mod llm_client;
pub mod observability;
pub mod openrouter;
pub mod runtime;
pub mod scoring;
pub mod strategy;
pub mod tools;
pub mod turn_ledger;

use graphbench_core::{AppError, ErrorCode, ErrorContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Init,
    Think,
    Act,
    Done,
}

impl LoopState {
    pub fn advance(self) -> Result<Self, AppError> {
        match self {
            Self::Init => Ok(Self::Think),
            Self::Think => Ok(Self::Act),
            Self::Act => Ok(Self::Done),
            Self::Done => Err(AppError::new(
                ErrorCode::ConfigurationInvalid,
                "loop is already complete",
                ErrorContext {
                    component: "harness",
                    operation: "advance_loop_state",
                },
            )),
        }
    }
}

pub use graph_tools::ensure_python_query_runtime_ready;
pub use runtime::{
    HarnessEvent, HarnessInput, HarnessModelResponse, HarnessOutput, HarnessRunConfig,
    HarnessRunner, ModelClient, ModelInvocation, ModelResponseKind, ObjectiveState,
    RefreshedGraphState, RuntimeLoopState, ToolCall,
};
pub use scoring::{
    DeterministicScoreBreakdown, ExplanationQualityInput, ExplanationQualityScore,
    ExplanationQualityScorer, FactRole, FactScore, JudgeAssistedSynthesisInput,
    JudgeAssistedSynthesisScore, JudgeAssistedSynthesisScorer, ProofObservation,
    UnscoredExplanationQuality, judge_synthesis, score_turn_ledger_deterministically,
    score_turn_ledger_with_explanation,
};
pub use strategy::{
    broad_graph_discovery, graph_then_targeted_lexical_read, high_recall_projection,
    minimal_projection, preset_by_id as strategy_preset_by_id, strict_no_reread,
};
pub use tools::{ToolCallTrace, ToolContract, ToolRegistry};
pub use turn_ledger::{TurnLedger, TurnLedgerEntry};

#[cfg(test)]
mod tests {
    use super::{
        LoopState,
        runtime::{
            HarnessInput, HarnessModelResponse, HarnessRunConfig, HarnessRunner, ModelClient,
            ModelResponseKind, ObjectiveState, RuntimeLoopState,
        },
        strategy::graph_then_targeted_lexical_read,
        tools::{ToolContract, ToolRegistry},
        turn_ledger::TurnLedger,
    };
    use graphbench_core::{
        GraphWorkspace, ReadinessState, RepresentationLevel, fixtures::FixtureRepository,
    };
    use serde_json::json;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    #[test]
    fn loop_state_advances_deterministically() {
        assert_eq!(
            LoopState::Init.advance().expect("init -> think"),
            LoopState::Think
        );
        assert_eq!(
            LoopState::Think.advance().expect("think -> act"),
            LoopState::Act
        );
        assert_eq!(
            LoopState::Act.advance().expect("act -> done"),
            LoopState::Done
        );
    }

    #[test]
    fn done_state_fails_fast() {
        assert!(LoopState::Done.advance().is_err());
    }

    struct MockModel {
        calls: usize,
    }

    impl ModelClient for MockModel {
        fn respond(
            &mut self,
            _prompt: &str,
        ) -> Result<super::ModelInvocation, graphbench_core::AppError> {
            self.calls += 1;
            let response = if self.calls == 1 {
                HarnessModelResponse {
                    kind: ModelResponseKind::ToolCall,
                    prompt_version: "v1".to_owned(),
                    model_slug: "mock-model".to_owned(),
                    provider: "mock-provider".to_owned(),
                    assistant_message: "Need file summary.".to_owned(),
                    tool_call: Some(super::ToolCall {
                        tool_name: "graph.describe".to_owned(),
                        payload: json!({ "selector": "README.md" }),
                    }),
                    acquired_fact_ids: vec!["fact-1".to_owned()],
                    readiness_state: ReadinessState::EvidenceAcquired,
                }
            } else {
                HarnessModelResponse {
                    kind: ModelResponseKind::Complete,
                    prompt_version: "v1".to_owned(),
                    model_slug: "mock-model".to_owned(),
                    provider: "mock-provider".to_owned(),
                    assistant_message: "Ready to stop.".to_owned(),
                    tool_call: None,
                    acquired_fact_ids: vec!["fact-2".to_owned()],
                    readiness_state: ReadinessState::ReadyToEdit,
                }
            };

            Ok(super::ModelInvocation {
                response,
                raw_request: json!({ "prompt": "mock" }),
                raw_response: json!({ "mock": true }),
                provider_request_id: Some(format!("mock-request-{}", self.calls)),
            })
        }
    }

    #[test]
    fn harness_executes_bounded_run_and_persists_replayable_ledger() {
        let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/graphbench-internal/fixture.json");
        let fixture_repository = FixtureRepository;
        let (fixture, resolution) = fixture_repository
            .load(&manifest_path)
            .expect("fixture should load");
        let workspace =
            GraphWorkspace::load(fixture.clone(), &resolution).expect("graph should load");
        let mut graph_session = workspace.session();
        graph_session.seed_overview(Some(2));
        graph_session
            .select(
                "crates/graphbench-core/src/artifacts.rs",
                RepresentationLevel::L1,
            )
            .expect("select");
        graph_session
            .hydrate_exact_proof("crates/graphbench-core/src/artifacts.rs", 2)
            .expect("hydrate");
        let hooks = graph_session.render_for_harness(1024);

        let input = HarnessInput {
            run_id: "run-1".to_owned(),
            fixture_id: fixture.fixture_id,
            task_id: "prepare-edit.schema-boundaries".to_owned(),
            objective: ObjectiveState {
                task_statement: "Gather evidence before editing.".to_owned(),
                task_class: "prepare_to_edit".to_owned(),
                allowed_tools: vec!["graph.describe".to_owned()],
                verification_targets: vec!["cargo test".to_owned()],
                unresolved_questions: Vec::new(),
            },
            graph_prompt: hooks,
            graph_session_snapshot: graph_session.session_json().expect("session json"),
            config: HarnessRunConfig {
                turn_budget: 3,
                timeout_ms: 5_000,
                token_budget: 4_000,
                prompt_headroom: 256,
                prompt_version: "v1".to_owned(),
                strategy: graph_then_targeted_lexical_read(),
                harness_version: "0.1.0".to_owned(),
            },
        };

        let mut model = MockModel { calls: 0 };
        let mut tools = ToolRegistry::default();
        tools.register(
            ToolContract {
                name: "graph.describe".to_owned(),
                version: "v1".to_owned(),
                input_description: "selector payload".to_owned(),
                output_description: "node summary".to_owned(),
            },
            |payload| payload.get("selector").is_some(),
            |output| output.get("summary").is_some(),
            |payload| {
                Ok(json!({
                    "summary": format!("described {}", payload["selector"].as_str().unwrap_or("unknown"))
                }))
            },
        );

        let events = Arc::new(Mutex::new(Vec::new()));
        let captured_events = Arc::clone(&events);
        let mut runner = HarnessRunner::new(&mut model, &tools).with_telemetry_hook(|event| {
            if let Ok(mut items) = captured_events.lock() {
                items.push(format!("{event:?}"));
            }
        });
        let output = runner.execute(&input).expect("run should complete");
        assert_eq!(output.final_state, RuntimeLoopState::Done);
        assert_eq!(output.turn_ledger.entries.len(), 2);
        assert_eq!(output.model_invocations.len(), 2);
        assert!(!events.lock().expect("events lock").is_empty());

        let path = std::env::temp_dir().join("graphbench-turn-ledger.json");
        output.turn_ledger.save(&path).expect("ledger should save");
        let restored = TurnLedger::load(&path).expect("ledger should load");
        restored.replay_validate().expect("ledger should replay");
        let _ = std::fs::remove_file(path);
    }
}
