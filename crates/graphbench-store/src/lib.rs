#![forbid(unsafe_code)]

pub mod run_store;

pub use run_store::{
    create_run_store, BlobReference, EvidenceMatchRecord, RunDetail, RunFilter, RunStore,
    SqliteRunStore, StoreSchemaVersion, TurnSummary,
};

#[cfg(test)]
mod tests {
    use crate::run_store::{self, create_run_store, BlobReference, EvidenceMatchRecord, RunFilter};
    use graphbench_core::artifacts::{
        ReadinessState, RunManifest, RunSchemaVersionSet, ScoreMetrics, ScoreReport,
        TelemetryCounts, TurnHashSet, TurnRequest, TurnResponse, TurnSelection,
    };
    use graphbench_core::strategy::{
        ContextWindowCompactionPolicy, ContextWindowStrategyPolicy, GraphDiscoveryMode,
        ProjectionMode, RereadMode, SectionTrimDirection, StrategyConfig, StrategySectionBudget,
        STRATEGY_CONFIG_SCHEMA_VERSION,
    };
    use std::fs;

    fn test_strategy_config() -> StrategyConfig {
        StrategyConfig {
            schema_version: STRATEGY_CONFIG_SCHEMA_VERSION,
            strategy_id: "graph.targeted-lexical-read".to_owned(),
            strategy_version: "v1".to_owned(),
            graph_discovery: GraphDiscoveryMode::GraphThenTargetedLexicalRead,
            projection: ProjectionMode::Balanced,
            reread_policy: RereadMode::Allow,
            context_window: ContextWindowStrategyPolicy {
                compaction: ContextWindowCompactionPolicy {
                    history_recent_items: 10,
                    summary_max_chars: 1000,
                    emergency_summary_max_chars: 500,
                    deduplicate_tool_results: true,
                },
                section_budgets: vec![StrategySectionBudget {
                    section_id: "selected_history".to_owned(),
                    max_tokens: 2000,
                    trim_direction: SectionTrimDirection::Tail,
                }],
            },
        }
    }

    fn test_manifest() -> RunManifest {
        RunManifest {
            run_id: "test-run-001".to_owned(),
            schema_version: 2,
            fixture_id: "test-fixture".to_owned(),
            task_id: "test-task".to_owned(),
            strategy_id: "graph.targeted-lexical-read".to_owned(),
            strategy_config: test_strategy_config(),
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

        let retrieved = store
            .get_run_manifest("test-run-001")
            .expect("get manifest");
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
        manifest1.strategy_id = "baseline.strategy-1".to_owned();
        manifest1.strategy_config.strategy_id = "baseline.strategy-1".to_owned();
        manifest1.outcome = "success".to_owned();
        store
            .store_run_manifest(&manifest1)
            .expect("store manifest 1");

        let mut manifest2 = test_manifest();
        manifest2.run_id = "run-2".to_owned();
        manifest2.fixture_id = "fixture-b".to_owned();
        manifest2.strategy_id = "baseline.strategy-2".to_owned();
        manifest2.strategy_config.strategy_id = "baseline.strategy-2".to_owned();
        manifest2.outcome = "failure".to_owned();
        store
            .store_run_manifest(&manifest2)
            .expect("store manifest 2");

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
        store
            .store_score_report(&report)
            .expect("store score report");

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

        store
            .store_turn_trace(&turn_trace)
            .expect("store turn trace");

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
        store
            .store_evidence_match(&record)
            .expect("store evidence match");

        let matches = store
            .get_evidence_matches("test-run-001")
            .expect("get matches");
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

    #[test]
    fn e2e_full_run_workflow() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-e2e-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");
        let run_id = "e2e-run-001";

        let manifest = RunManifest {
            run_id: run_id.to_owned(),
            schema_version: 2,
            fixture_id: "graphbench.internal".to_owned(),
            task_id: "task.prepare-edit-01".to_owned(),
            strategy_id: "graph.targeted-lexical-read".to_owned(),
            strategy_config: test_strategy_config(),
            harness_version: "0.1.0".to_owned(),
            schema_version_set: RunSchemaVersionSet {
                fixture_manifest: 1,
                task_spec: 1,
                evidence_spec: 1,
                strategy_config: 1,
                context_object: 1,
                context_window_section: 1,
                turn_trace: 1,
                score_report: 1,
            },
            provider: "openrouter".to_owned(),
            model_slug: "anthropic/claude-3-sonnet".to_owned(),
            prompt_version: "v1".to_owned(),
            graph_snapshot_id: "sha256:".to_owned() + &"a".repeat(64),
            started_at: "2024-01-15T10:00:00Z".to_owned(),
            completed_at: "2024-01-15T10:05:00Z".to_owned(),
            outcome: "success".to_owned(),
        };
        store.store_run_manifest(&manifest).expect("store manifest");

        for i in 0..3 {
            let turn_trace = graphbench_core::artifacts::TurnTrace {
                run_id: run_id.to_owned(),
                turn_index: i,
                task_id: "task.prepare-edit-01".to_owned(),
                fixture_id: "graphbench.internal".to_owned(),
                strategy_id: "graph.targeted-lexical-read".to_owned(),
                request: TurnRequest {
                    schema_version: 1,
                    prompt_version: "v1".to_owned(),
                    prompt_hash: format!("sha256:{:0>64}", format!("{:x}", i)),
                    context_hash: format!("sha256:{:0>64}", format!("{:x}", i + 100)),
                },
                response: TurnResponse {
                    provider: "openrouter".to_owned(),
                    model_slug: "anthropic/claude-3-sonnet".to_owned(),
                    schema_version: 1,
                    validated: true,
                },
                selection: TurnSelection {
                    selected_context_objects: vec![format!("ctx-{}", i)],
                    omitted_candidates: vec![],
                    rendered_sections: vec![],
                },
                telemetry: TelemetryCounts {
                    prompt_bytes: 1000 + i * 100,
                    prompt_tokens: 250 + i * 25,
                    latency_ms: 1000 + i * 50,
                    tool_calls: 2 + i,
                },
                evidence_delta: if i == 1 {
                    vec!["fact-1".to_owned(), "fact-2".to_owned()]
                } else {
                    vec![]
                },
                readiness_state: if i == 2 {
                    ReadinessState::ReadyToEdit
                } else {
                    ReadinessState::NotReady
                },
                readiness_reason: if i == 2 {
                    "all required evidence acquired".to_owned()
                } else {
                    "still gathering evidence".to_owned()
                },
                hashes: TurnHashSet {
                    turn_hash: format!("sha256:{:0>64}", format!("{:x}", i + 1000)),
                },
            };
            store.store_turn_trace(&turn_trace).expect("store turn");

            if i == 1 {
                store
                    .store_evidence_match(&EvidenceMatchRecord {
                        run_id: run_id.to_owned(),
                        turn_index: 1,
                        fact_id: "fact-1".to_owned(),
                        matched_at: "2024-01-15T10:02:00Z".to_owned(),
                    })
                    .expect("store evidence match 1");
                store
                    .store_evidence_match(&EvidenceMatchRecord {
                        run_id: run_id.to_owned(),
                        turn_index: 1,
                        fact_id: "fact-2".to_owned(),
                        matched_at: "2024-01-15T10:02:30Z".to_owned(),
                    })
                    .expect("store evidence match 2");
            }
        }

        let score_report = ScoreReport {
            run_id: run_id.to_owned(),
            task_id: "task.prepare-edit-01".to_owned(),
            schema_version: 1,
            evidence_visibility_score: 0.95,
            evidence_acquisition_score: 0.90,
            evidence_efficiency_score: 0.85,
            explanation_quality_score: 0.88,
            metrics: ScoreMetrics {
                required_evidence_recall: 1.0,
                evidence_precision: 0.95,
                irrelevant_material_ratio: 0.05,
                turns_to_readiness: 2,
                reread_count: 0,
                post_readiness_drift_turns: 0,
            },
        };
        store
            .store_score_report(&score_report)
            .expect("store score report");

        store
            .store_blob_reference(&BlobReference {
                blob_id: "sha256:abc123".to_owned(),
                run_id: run_id.to_owned(),
                turn_index: Some(0),
                blob_type: "prompt".to_owned(),
                media_type: "text/plain".to_owned(),
                path: "/blobs/prompt-0.txt".to_owned(),
                byte_count: 1000,
            })
            .expect("store blob ref");

        let retrieved_manifest = store
            .get_run_manifest(run_id)
            .expect("get manifest")
            .expect("manifest exists");
        assert_eq!(retrieved_manifest.run_id, run_id);
        assert_eq!(retrieved_manifest.fixture_id, "graphbench.internal");

        let turns = store.list_turns(run_id).expect("list turns");
        assert_eq!(turns.len(), 3);
        assert_eq!(turns[2].turn_index, 2);
        assert_eq!(turns[2].readiness_state, ReadinessState::ReadyToEdit);

        let matches = store
            .get_evidence_matches(run_id)
            .expect("get evidence matches");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].fact_id, "fact-1");

        let report = store
            .get_score_report(run_id)
            .expect("get score report")
            .expect("report exists");
        assert!((report.evidence_visibility_score - 0.95).abs() < 0.001);
        assert_eq!(report.metrics.turns_to_readiness, 2);

        let blobs = store.get_blob_references(run_id).expect("get blobs");
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].blob_type, "prompt");

        let runs = store.list_runs(None).expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].turn_count, 3);

        let detail = run_store::sqlite::get_run_detail(store.as_ref(), run_id)
            .expect("get detail")
            .expect("detail exists");
        assert_eq!(detail.turns.len(), 3);
        assert!(detail.score_report.is_some());
        assert_eq!(detail.evidence_matches.len(), 2);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn e2e_run_comparison_workflow() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-compare-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");

        let strategies = vec![
            ("baseline.broad-discovery", "baseline.broad-discovery"),
            ("graph.targeted-lexical-read", "graph.targeted-lexical-read"),
        ];

        for (i, (strategy_id, _)) in strategies.iter().enumerate() {
            let run_id = format!("compare-run-{:03}", i);
            let mut manifest = test_manifest();
            manifest.run_id = run_id.clone();
            manifest.strategy_id = strategy_id.to_string();
            manifest.strategy_config.strategy_id = strategy_id.to_string();
            manifest.outcome = if i == 0 { "success" } else { "success" }.to_owned();
            store.store_run_manifest(&manifest).expect("store manifest");

            let report = ScoreReport {
                run_id: run_id.clone(),
                task_id: "task-1".to_owned(),
                schema_version: 1,
                evidence_visibility_score: 0.8 + (i as f64 * 0.1),
                evidence_acquisition_score: 0.75 + (i as f64 * 0.1),
                evidence_efficiency_score: 0.7 + (i as f64 * 0.1),
                explanation_quality_score: 0.85,
                metrics: ScoreMetrics {
                    required_evidence_recall: 0.9,
                    evidence_precision: 0.88,
                    irrelevant_material_ratio: 0.12,
                    turns_to_readiness: 3,
                    reread_count: i as u32,
                    post_readiness_drift_turns: 0,
                },
            };
            store.store_score_report(&report).expect("store report");
        }

        let runs = store
            .list_runs(Some(RunFilter {
                strategy_id: Some("baseline.broad-discovery".to_owned()),
                ..Default::default()
            }))
            .expect("filter runs");
        assert_eq!(runs.len(), 1);

        let all_runs = store.list_runs(None).expect("list all runs");
        assert_eq!(all_runs.len(), 2);

        for run in all_runs {
            let report = store
                .get_score_report(&run.run_id)
                .expect("get report")
                .expect("report exists");
            println!(
                "Strategy: {}, Visibility: {:.2}",
                run.strategy_id, report.evidence_visibility_score
            );
        }

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn e2e_replay_from_store() {
        let temp_dir = std::env::temp_dir().join("graphbench-store-replay-test");
        let db_path = temp_dir.join("test.db");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir");

        let store = create_run_store(&db_path).expect("create store");
        let run_id = "replay-test-001";

        let mut manifest = test_manifest();
        manifest.run_id = run_id.to_owned();
        store.store_run_manifest(&manifest).expect("store manifest");

        let turn_trace = graphbench_core::artifacts::TurnTrace {
            run_id: run_id.to_owned(),
            turn_index: 0,
            task_id: "test-task".to_owned(),
            fixture_id: "test-fixture".to_owned(),
            strategy_id: "graph.targeted-lexical-read".to_owned(),
            request: TurnRequest {
                schema_version: 1,
                prompt_version: "v1".to_owned(),
                prompt_hash: "sha256:".to_owned() + &"a".repeat(64),
                context_hash: "sha256:".to_owned() + &"b".repeat(64),
            },
            response: TurnResponse {
                provider: "test".to_owned(),
                model_slug: "model".to_owned(),
                schema_version: 1,
                validated: true,
            },
            selection: TurnSelection {
                selected_context_objects: vec!["ctx-replay-1".to_owned()],
                omitted_candidates: vec![],
                rendered_sections: vec![],
            },
            telemetry: TelemetryCounts {
                prompt_bytes: 500,
                prompt_tokens: 125,
                latency_ms: 500,
                tool_calls: 1,
            },
            evidence_delta: vec!["fact-replay-1".to_owned()],
            readiness_state: ReadinessState::EvidenceAcquired,
            readiness_reason: "evidence gathered".to_owned(),
            hashes: TurnHashSet {
                turn_hash: "sha256:".to_owned() + &"c".repeat(64),
            },
        };
        store.store_turn_trace(&turn_trace).expect("store turn");

        let turns = store.list_turns(run_id).expect("list turns for replay");
        assert_eq!(turns.len(), 1);

        let retrieved = &turns[0];
        assert_eq!(
            retrieved.request.prompt_hash,
            "sha256:".to_owned() + &"a".repeat(64)
        );
        assert_eq!(retrieved.evidence_delta.len(), 1);
        assert_eq!(retrieved.evidence_delta[0], "fact-replay-1");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
