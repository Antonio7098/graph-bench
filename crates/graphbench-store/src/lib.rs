#![forbid(unsafe_code)]

pub mod run_store;

pub use run_store::{
    create_run_store, BlobReference, EvidenceMatchRecord, RunDetail, RunFilter, RunStore,
    RunSummary, RunSummary, SqliteRunStore, StoreSchemaVersion, TurnSummary,
};

#[cfg(test)]
mod tests {
    use crate::run_store::{create_run_store, RunFilter};
    use graphbench_core::artifacts::{
        ReadinessState, RunManifest, RunSchemaVersionSet, ScoreMetrics, ScoreReport,
        TelemetryCounts, TurnHashSet, TurnRequest, TurnResponse, TurnSelection,
    };
    use graphbench_core::strategy::STRATEGY_CONFIG_SCHEMA_VERSION;
    use std::fs;

    fn test_manifest() -> RunManifest {
        RunManifest {
            run_id: "test-run-001".to_owned(),
            schema_version: 2,
            fixture_id: "test-fixture".to_owned(),
            task_id: "test-task".to_owned(),
            strategy_id: "graph.targeted-lexical-read".to_owned(),
            strategy_config: graphbench_core::strategy::graph_then_targeted_lexical_read(),
            harness_version: "0.1.0".to_owned(),
            schema_version_set: RunSchemaVersionSet {
                fixture_manifest: 1,
                task_spec: 1,
                evidence_spec: 1,
                strategy_config: STRATEGY_CONFIG_SCHEMA_VERSION,
                context_object: 1,
                context_window_section: 1,
                turn_trace: 1,
                score_report: 1,
            },
            provider: "test-provider".to_owned(),
            model_slug: "test-model".to_owned(),
            prompt_version: "v1".to_owned(),
            graph_snapshot_id: "sha256:".to_owned() + &"a".repeat(64),
            started_at: "2024-01-01T00:00:00Z".to_owned(),
            completed_at: "2024-01-01T00:01:00Z".to_owned(),
            outcome: "success".to_owned(),
        }
    }

    fn test_score_report(run_id: &str) -> ScoreReport {
        ScoreReport {
            run_id: run_id.to_owned(),
            task_id: "test-task".to_owned(),
            schema_version: 1,
            evidence_visibility_score: 0.9,
            evidence_acquisition_score: 0.85,
            evidence_efficiency_score: 0.8,
            explanation_quality_score: 0.75,
            metrics: ScoreMetrics {
                required_evidence_recall: 0.95,
                evidence_precision: 0.88,
                irrelevant_material_ratio: 0.12,
                turns_to_readiness: 3,
                reread_count: 1,
                post_readiness_drift_turns: 0,
            },
        }
    }

    #[test]
    fn sqlite_store_manages_run_lifecycle() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");

        let manifest = test_manifest();
        store.store_run_manifest(&manifest).expect("store manifest");

        let retrieved = store.get_run_manifest("test-run-001").expect("get manifest");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.run_id, "test-run-001");
        assert_eq!(retrieved.fixture_id, "test-fixture");

        let runs = store.list_runs(None).expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "test-run-001");

        store.delete_run("test-run-001").expect("delete run");
        let runs = store.list_runs(None).expect("list runs after delete");
        assert!(runs.is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn sqlite_store_filters_runs() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-filter-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");

        let mut manifest1 = test_manifest();
        manifest1.run_id = "run-1".to_owned();
        manifest1.fixture_id = "fixture-a".to_owned();
        manifest1.strategy_id = "strategy-1".to_owned();
        manifest1.outcome = "success".to_owned();
        store.store_run_manifest(&manifest1).expect("store manifest 1");

        let mut manifest2 = test_manifest();
        manifest2.run_id = "run-2".to_owned();
        manifest2.fixture_id = "fixture-b".to_owned();
        manifest2.strategy_id = "strategy-2".to_owned();
        manifest2.outcome = "failure".to_owned();
        store.store_run_manifest(&manifest2).expect("store manifest 2");

        let runs = store
            .list_runs(Some(RunFilter {
                fixture_id: Some("fixture-a".to_owned()),
                ..Default::default()
            }))
            .expect("filter by fixture");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run-1");

        let runs = store
            .list_runs(Some(RunFilter {
                outcome: Some("failure".to_owned()),
                ..Default::default()
            }))
            .expect("filter by outcome");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run-2");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn sqlite_store_persists_score_reports() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-score-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");

        let manifest = test_manifest();
        store.store_run_manifest(&manifest).expect("store manifest");

        let report = test_score_report("test-run-001");
        store.store_score_report(&report).expect("store score report");

        let retrieved = store.get_score_report("test-run-001").expect("get report");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.evidence_visibility_score, 0.9);
        assert_eq!(retrieved.metrics.turns_to_readiness, 3);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn sqlite_store_persists_turn_traces() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-turns-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");

        let manifest = test_manifest();
        store.store_run_manifest(&manifest).expect("store manifest");

        use graphbench_core::artifacts::{ContextObject, ContextObjectHashSet, ContextIdentity, ContextProvenance, RepresentationLevel, LeaseState, EvidenceMatch};

        let turn_trace = graphbench_core::artifacts::TurnTrace {
            run_id: "test-run-001".to_owned(),
            turn_index: 0,
            task_id: "test-task".to_owned(),
            fixture_id: "test-fixture".to_owned(),
            strategy_id: "graph.targeted-lexical-read".to_owned(),
            request: TurnRequest {
                schema_version: 1,
                prompt_version: "v1".to_owned(),
                prompt_hash: "sha256:".to_owned() + &"b".repeat(64),
                context_hash: "sha256:".to_owned() + &"c".repeat(64),
            },
            response: TurnResponse {
                provider: "test".to_owned(),
                model_slug: "model".to_owned(),
                schema_version: 1,
                validated: true,
            },
            selection: TurnSelection {
                selected_context_objects: vec!["ctx-1".to_owned()],
                omitted_candidates: vec![],
                rendered_sections: vec![],
            },
            telemetry: TelemetryCounts {
                prompt_bytes: 100,
                prompt_tokens: 25,
                latency_ms: 1000,
                tool_calls: 2,
            },
            evidence_delta: vec!["fact-1".to_owned()],
            readiness_state: ReadinessState::EvidenceAcquired,
            readiness_reason: "required facts were gathered".to_owned(),
            hashes: TurnHashSet {
                turn_hash: "sha256:".to_owned() + &"d".repeat(64),
            },
        };

        store.store_turn_trace(&turn_trace).expect("store turn trace");

        let turns = store.list_turns("test-run-001").expect("list turns");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].turn_index, 0);

        let retrieved = store.get_turn_trace("test-run-001", 0).expect("get turn");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().evidence_delta.len(), 1);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn sqlite_store_persists_evidence_matches() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-evidence-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");

        let manifest = test_manifest();
        store.store_run_manifest(&manifest).expect("store manifest");

        let record = crate::run_store::EvidenceMatchRecord {
            run_id: "test-run-001".to_owned(),
            turn_index: 0,
            fact_id: "fact-1".to_owned(),
            matched_at: "2024-01-01T00:00:30Z".to_owned(),
        };
        store.store_evidence_match(&record).expect("store evidence match");

        let matches = store.get_evidence_matches("test-run-001").expect("get matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].fact_id, "fact-1");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn sqlite_store_tracks_version() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-version-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");

        let version = store.get_store_version().expect("get version");
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
