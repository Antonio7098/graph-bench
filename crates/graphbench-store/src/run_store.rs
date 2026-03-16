use graphbench_core::artifacts::{RunManifest, ScoreReport, TurnTrace};
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceMatchRecord {
    pub run_id: String,
    pub turn_index: u32,
    pub fact_id: String,
    pub matched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobReference {
    pub blob_id: String,
    pub run_id: String,
    pub turn_index: Option<u32>,
    pub blob_type: String,
    pub media_type: String,
    pub path: String,
    pub byte_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSchemaVersion {
    pub major: u32,
    pub minor: u32,
    pub applied_at: String,
}

pub trait RunStore: Send + Sync {
    fn store_run_manifest(&self, manifest: &RunManifest) -> Result<(), AppError>;
    fn get_run_manifest(&self, run_id: &str) -> Result<Option<RunManifest>, AppError>;
    fn list_runs(&self, filter: Option<RunFilter>) -> Result<Vec<RunSummary>, AppError>;
    fn delete_run(&self, run_id: &str) -> Result<(), AppError>;

    fn store_turn_trace(&self, trace: &TurnTrace) -> Result<(), AppError>;
    fn get_turn_trace(&self, run_id: &str, turn_index: u32) -> Result<Option<TurnTrace>, AppError>;
    fn list_turns(&self, run_id: &str) -> Result<Vec<TurnTrace>, AppError>;

    fn store_evidence_match(&self, record: &EvidenceMatchRecord) -> Result<(), AppError>;
    fn get_evidence_matches(&self, run_id: &str) -> Result<Vec<EvidenceMatchRecord>, AppError>;

    fn store_score_report(&self, report: &ScoreReport) -> Result<(), AppError>;
    fn get_score_report(&self, run_id: &str) -> Result<Option<ScoreReport>, AppError>;

    fn store_blob_reference(&self, reference: &BlobReference) -> Result<(), AppError>;
    fn get_blob_references(&self, run_id: &str) -> Result<Vec<BlobReference>, AppError>;

    fn get_store_version(&self) -> Result<StoreSchemaVersion, AppError>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunFilter {
    pub fixture_id: Option<String>,
    pub task_id: Option<String>,
    pub strategy_id: Option<String>,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub fixture_id: String,
    pub task_id: String,
    pub strategy_id: String,
    pub provider: String,
    pub model_slug: String,
    pub started_at: String,
    pub completed_at: String,
    pub outcome: String,
    pub turn_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSummary {
    pub run_id: String,
    pub turn_index: u32,
    pub task_id: String,
    pub readiness_state: String,
    pub evidence_delta_count: u32,
    pub prompt_tokens: u32,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDetail {
    pub manifest: RunManifest,
    pub turns: Vec<TurnTrace>,
    pub evidence_matches: Vec<EvidenceMatchRecord>,
    pub score_report: Option<ScoreReport>,
    pub blob_references: Vec<BlobReference>,
}

pub fn create_run_store(path: impl AsRef<Path>) -> Result<Box<dyn RunStore>, AppError> {
    SqliteRunStore::new(path)
}

pub mod sqlite {
    use super::*;
    use rusqlite::{params, Connection};
    use std::path::Path;
    use std::sync::Mutex;

    const STORE_MAJOR_VERSION: u32 = 1;
    const STORE_MINOR_VERSION: u32 = 0;

    pub struct SqliteRunStore {
        conn: Mutex<Connection>,
    }

    impl SqliteRunStore {
        pub fn new(path: impl AsRef<Path>) -> Result<Box<dyn RunStore>, AppError> {
            let conn = Connection::open(path.as_ref()).map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to open SQLite database",
                    ErrorContext {
                        component: "run_store",
                        operation: "open",
                    },
                    source,
                )
            })?;

            let store = Self {
                conn: Mutex::new(conn),
            };
            store.init_schema()?;
            Ok(Box::new(store))
        }

        fn init_schema(&self) -> Result<(), AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "init_schema",
                    },
                )
            })?;

            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS store_version (
                    major INTEGER NOT NULL,
                    minor INTEGER NOT NULL,
                    applied_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS runs (
                    run_id TEXT PRIMARY KEY,
                    fixture_id TEXT NOT NULL,
                    task_id TEXT NOT NULL,
                    strategy_id TEXT NOT NULL,
                    strategy_config TEXT NOT NULL,
                    harness_version TEXT NOT NULL,
                    schema_version_set TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model_slug TEXT NOT NULL,
                    prompt_version TEXT NOT NULL,
                    graph_snapshot_id TEXT NOT NULL,
                    started_at TEXT NOT NULL,
                    completed_at TEXT NOT NULL,
                    outcome TEXT NOT NULL,
                    manifest_json TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS turns (
                    run_id TEXT NOT NULL,
                    turn_index INTEGER NOT NULL,
                    task_id TEXT NOT NULL,
                    fixture_id TEXT NOT NULL,
                    strategy_id TEXT NOT NULL,
                    trace_json TEXT NOT NULL,
                    PRIMARY KEY (run_id, turn_index),
                    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS evidence_matches (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    run_id TEXT NOT NULL,
                    turn_index INTEGER NOT NULL,
                    fact_id TEXT NOT NULL,
                    matched_at TEXT NOT NULL,
                    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS score_reports (
                    run_id TEXT PRIMARY KEY,
                    report_json TEXT NOT NULL,
                    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS blob_references (
                    blob_id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    turn_index INTEGER,
                    blob_type TEXT NOT NULL,
                    media_type TEXT NOT NULL,
                    path TEXT NOT NULL,
                    byte_count INTEGER NOT NULL,
                    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_runs_fixture ON runs(fixture_id);
                CREATE INDEX IF NOT EXISTS idx_runs_task ON runs(task_id);
                CREATE INDEX IF NOT EXISTS idx_runs_strategy ON runs(strategy_id);
                CREATE INDEX IF NOT EXISTS idx_runs_outcome ON runs(outcome);
                CREATE INDEX IF NOT EXISTS idx_turns_run ON turns(run_id);
                CREATE INDEX IF NOT EXISTS idx_evidence_run ON evidence_matches(run_id);
                CREATE INDEX IF NOT EXISTS idx_blobs_run ON blob_references(run_id);
                "#,
            )
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to initialize database schema",
                    ErrorContext {
                        component: "run_store",
                        operation: "init_schema",
                    },
                    source,
                )
            })?;

            let version_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM store_version", [], |row| row.get(0))
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to check store version",
                        ErrorContext {
                            component: "run_store",
                            operation: "init_schema",
                        },
                        source,
                    )
                })?;

            if version_count == 0 {
                let now = chrono::Utc::now().to_rfc3339();
                conn.execute(
                    "INSERT INTO store_version (major, minor, applied_at) VALUES (?1, ?2, ?3)",
                    params![STORE_MAJOR_VERSION, STORE_MINOR_VERSION, now],
                )
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to insert store version",
                        ErrorContext {
                            component: "run_store",
                            operation: "init_schema",
                        },
                        source,
                    )
                })?;
            }

            Ok(())
        }
    }

    impl RunStore for SqliteRunStore {
        fn store_run_manifest(&self, manifest: &RunManifest) -> Result<(), AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_run_manifest",
                    },
                )
            })?;

            manifest.validate()?;

            let config_json =
                serde_json::to_string(&manifest.strategy_config).map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to serialize strategy config",
                        ErrorContext {
                            component: "run_store",
                            operation: "store_run_manifest",
                        },
                        source,
                    )
                })?;

            let schema_versions_json = serde_json::to_string(&manifest.schema_version_set)
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to serialize schema versions",
                        ErrorContext {
                            component: "run_store",
                            operation: "store_run_manifest",
                        },
                        source,
                    )
                })?;

            let manifest_json = serde_json::to_string(manifest).map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to serialize run manifest",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_run_manifest",
                    },
                    source,
                )
            })?;

            conn.execute(
                r#"INSERT OR REPLACE INTO runs 
                   (run_id, fixture_id, task_id, strategy_id, strategy_config, harness_version,
                    schema_version_set, provider, model_slug, prompt_version, graph_snapshot_id,
                    started_at, completed_at, outcome, manifest_json)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
                params![
                    manifest.run_id,
                    manifest.fixture_id,
                    manifest.task_id,
                    manifest.strategy_id,
                    config_json,
                    manifest.harness_version,
                    schema_versions_json,
                    manifest.provider,
                    manifest.model_slug,
                    manifest.prompt_version,
                    manifest.graph_snapshot_id,
                    manifest.started_at,
                    manifest.completed_at,
                    manifest.outcome,
                    manifest_json,
                ],
            )
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to store run manifest",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_run_manifest",
                    },
                    source,
                )
            })?;

            Ok(())
        }

        fn get_run_manifest(&self, run_id: &str) -> Result<Option<RunManifest>, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_run_manifest",
                    },
                )
            })?;

            let result = conn.query_row(
                "SELECT manifest_json FROM runs WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(json) => {
                    let manifest: RunManifest = serde_json::from_str(&json).map_err(|source| {
                        AppError::with_source(
                            ErrorCode::SchemaValidationFailed,
                            "failed to deserialize run manifest",
                            ErrorContext {
                                component: "run_store",
                                operation: "get_run_manifest",
                            },
                            source,
                        )
                    })?;
                    Ok(Some(manifest))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(source) => Err(AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to retrieve run manifest",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_run_manifest",
                    },
                    source,
                )),
            }
        }

        fn list_runs(&self, filter: Option<RunFilter>) -> Result<Vec<RunSummary>, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "list_runs",
                    },
                )
            })?;

            let mut sql = String::from(
                "SELECT r.run_id, r.fixture_id, r.task_id, r.strategy_id, r.provider, r.model_slug, r.started_at, r.completed_at, r.outcome, COUNT(t.turn_index) as turn_count FROM runs r LEFT JOIN turns t ON r.run_id = t.run_id",
            );
            let mut conditions = Vec::new();
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(ref f) = filter {
                if let Some(ref fixture_id) = f.fixture_id {
                    conditions.push("r.fixture_id = ?");
                    params_vec.push(Box::new(fixture_id.clone()));
                }
                if let Some(ref task_id) = f.task_id {
                    conditions.push("r.task_id = ?");
                    params_vec.push(Box::new(task_id.clone()));
                }
                if let Some(ref strategy_id) = f.strategy_id {
                    conditions.push("r.strategy_id = ?");
                    params_vec.push(Box::new(strategy_id.clone()));
                }
                if let Some(ref outcome) = f.outcome {
                    conditions.push("r.outcome = ?");
                    params_vec.push(Box::new(outcome.clone()));
                }
            }

            if !conditions.is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(&conditions.join(" AND "));
            }

            sql.push_str(" GROUP BY r.run_id ORDER BY r.started_at DESC");

            let mut stmt = conn.prepare(&sql).map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to prepare run list query",
                    ErrorContext {
                        component: "run_store",
                        operation: "list_runs",
                    },
                    source,
                )
            })?;

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    Ok(RunSummary {
                        run_id: row.get(0)?,
                        fixture_id: row.get(1)?,
                        task_id: row.get(2)?,
                        strategy_id: row.get(3)?,
                        provider: row.get(4)?,
                        model_slug: row.get(5)?,
                        started_at: row.get(6)?,
                        completed_at: row.get(7)?,
                        outcome: row.get(8)?,
                        turn_count: row.get(9)?,
                    })
                })
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to query runs",
                        ErrorContext {
                            component: "run_store",
                            operation: "list_runs",
                        },
                        source,
                    )
                })?;

            let mut summaries = Vec::new();
            for row in rows {
                summaries.push(row.map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to read run row",
                        ErrorContext {
                            component: "run_store",
                            operation: "list_runs",
                        },
                        source,
                    )
                })?);
            }

            Ok(summaries)
        }

        fn delete_run(&self, run_id: &str) -> Result<(), AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "delete_run",
                    },
                )
            })?;

            conn.execute("DELETE FROM runs WHERE run_id = ?1", [run_id])
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to delete run",
                        ErrorContext {
                            component: "run_store",
                            operation: "delete_run",
                        },
                        source,
                    )
                })?;

            Ok(())
        }

        fn store_turn_trace(&self, trace: &TurnTrace) -> Result<(), AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_turn_trace",
                    },
                )
            })?;

            trace.validate_for_persistence()?;

            let trace_json = serde_json::to_string(trace).map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to serialize turn trace",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_turn_trace",
                    },
                    source,
                )
            })?;

            conn.execute(
                r#"INSERT OR REPLACE INTO turns 
                   (run_id, turn_index, task_id, fixture_id, strategy_id, trace_json)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                params![
                    trace.run_id,
                    trace.turn_index,
                    trace.task_id,
                    trace.fixture_id,
                    trace.strategy_id,
                    trace_json,
                ],
            )
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to store turn trace",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_turn_trace",
                    },
                    source,
                )
            })?;

            Ok(())
        }

        fn get_turn_trace(
            &self,
            run_id: &str,
            turn_index: u32,
        ) -> Result<Option<TurnTrace>, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_turn_trace",
                    },
                )
            })?;

            let result = conn.query_row(
                "SELECT trace_json FROM turns WHERE run_id = ?1 AND turn_index = ?2",
                params![run_id, turn_index],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(json) => {
                    let trace: TurnTrace = serde_json::from_str(&json).map_err(|source| {
                        AppError::with_source(
                            ErrorCode::SchemaValidationFailed,
                            "failed to deserialize turn trace",
                            ErrorContext {
                                component: "run_store",
                                operation: "get_turn_trace",
                            },
                            source,
                        )
                    })?;
                    Ok(Some(trace))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(source) => Err(AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to retrieve turn trace",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_turn_trace",
                    },
                    source,
                )),
            }
        }

        fn list_turns(&self, run_id: &str) -> Result<Vec<TurnTrace>, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "list_turns",
                    },
                )
            })?;

            let mut stmt = conn
                .prepare("SELECT trace_json FROM turns WHERE run_id = ?1 ORDER BY turn_index")
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to prepare turns query",
                        ErrorContext {
                            component: "run_store",
                            operation: "list_turns",
                        },
                        source,
                    )
                })?;

            let rows = stmt
                .query_map([run_id], |row| row.get::<_, String>(0))
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to query turns",
                        ErrorContext {
                            component: "run_store",
                            operation: "list_turns",
                        },
                        source,
                    )
                })?;

            let mut traces = Vec::new();
            for row in rows {
                let json = row.map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to read turn row",
                        ErrorContext {
                            component: "run_store",
                            operation: "list_turns",
                        },
                        source,
                    )
                })?;
                let trace: TurnTrace = serde_json::from_str(&json).map_err(|source| {
                    AppError::with_source(
                        ErrorCode::SchemaValidationFailed,
                        "failed to deserialize turn trace",
                        ErrorContext {
                            component: "run_store",
                            operation: "list_turns",
                        },
                        source,
                    )
                })?;
                traces.push(trace);
            }

            Ok(traces)
        }

        fn store_evidence_match(&self, record: &EvidenceMatchRecord) -> Result<(), AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_evidence_match",
                    },
                )
            })?;

            conn.execute(
                r#"INSERT INTO evidence_matches (run_id, turn_index, fact_id, matched_at)
                   VALUES (?1, ?2, ?3, ?4)"#,
                params![
                    record.run_id,
                    record.turn_index,
                    record.fact_id,
                    record.matched_at
                ],
            )
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to store evidence match",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_evidence_match",
                    },
                    source,
                )
            })?;

            Ok(())
        }

        fn get_evidence_matches(&self, run_id: &str) -> Result<Vec<EvidenceMatchRecord>, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_evidence_matches",
                    },
                )
            })?;

            let mut stmt = conn
                .prepare("SELECT run_id, turn_index, fact_id, matched_at FROM evidence_matches WHERE run_id = ?1 ORDER BY turn_index, fact_id")
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to prepare evidence matches query",
                        ErrorContext {
                            component: "run_store",
                            operation: "get_evidence_matches",
                        },
                        source,
                    )
                })?;

            let rows = stmt
                .query_map([run_id], |row| {
                    Ok(EvidenceMatchRecord {
                        run_id: row.get(0)?,
                        turn_index: row.get(1)?,
                        fact_id: row.get(2)?,
                        matched_at: row.get(3)?,
                    })
                })
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to query evidence matches",
                        ErrorContext {
                            component: "run_store",
                            operation: "get_evidence_matches",
                        },
                        source,
                    )
                })?;

            let mut matches = Vec::new();
            for row in rows {
                matches.push(row.map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to read evidence match row",
                        ErrorContext {
                            component: "run_store",
                            operation: "get_evidence_matches",
                        },
                        source,
                    )
                })?);
            }

            Ok(matches)
        }

        fn store_score_report(&self, report: &ScoreReport) -> Result<(), AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_score_report",
                    },
                )
            })?;

            report.validate_for_persistence()?;

            let report_json = serde_json::to_string(report).map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to serialize score report",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_score_report",
                    },
                    source,
                )
            })?;

            conn.execute(
                "INSERT OR REPLACE INTO score_reports (run_id, report_json) VALUES (?1, ?2)",
                params![report.run_id, report_json],
            )
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to store score report",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_score_report",
                    },
                    source,
                )
            })?;

            Ok(())
        }

        fn get_score_report(&self, run_id: &str) -> Result<Option<ScoreReport>, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_score_report",
                    },
                )
            })?;

            let result = conn.query_row(
                "SELECT report_json FROM score_reports WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(json) => {
                    let report: ScoreReport = serde_json::from_str(&json).map_err(|source| {
                        AppError::with_source(
                            ErrorCode::SchemaValidationFailed,
                            "failed to deserialize score report",
                            ErrorContext {
                                component: "run_store",
                                operation: "get_score_report",
                            },
                            source,
                        )
                    })?;
                    Ok(Some(report))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(source) => Err(AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to retrieve score report",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_score_report",
                    },
                    source,
                )),
            }
        }

        fn store_blob_reference(&self, reference: &BlobReference) -> Result<(), AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_blob_reference",
                    },
                )
            })?;

            conn.execute(
                r#"INSERT OR REPLACE INTO blob_references 
                   (blob_id, run_id, turn_index, blob_type, media_type, path, byte_count)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![
                    reference.blob_id,
                    reference.run_id,
                    reference.turn_index,
                    reference.blob_type,
                    reference.media_type,
                    reference.path,
                    reference.byte_count as i64,
                ],
            )
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to store blob reference",
                    ErrorContext {
                        component: "run_store",
                        operation: "store_blob_reference",
                    },
                    source,
                )
            })?;

            Ok(())
        }

        fn get_blob_references(&self, run_id: &str) -> Result<Vec<BlobReference>, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_blob_references",
                    },
                )
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT blob_id, run_id, turn_index, blob_type, media_type, path, byte_count FROM blob_references WHERE run_id = ?1",
                )
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to prepare blob references query",
                        ErrorContext {
                            component: "run_store",
                            operation: "get_blob_references",
                        },
                        source,
                    )
                })?;

            let rows = stmt
                .query_map([run_id], |row| {
                    Ok(BlobReference {
                        blob_id: row.get(0)?,
                        run_id: row.get(1)?,
                        turn_index: row.get(2)?,
                        blob_type: row.get(3)?,
                        media_type: row.get(4)?,
                        path: row.get(5)?,
                        byte_count: row.get::<_, i64>(6)? as u64,
                    })
                })
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to query blob references",
                        ErrorContext {
                            component: "run_store",
                            operation: "get_blob_references",
                        },
                        source,
                    )
                })?;

            let mut refs = Vec::new();
            for row in rows {
                refs.push(row.map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to read blob reference row",
                        ErrorContext {
                            component: "run_store",
                            operation: "get_blob_references",
                        },
                        source,
                    )
                })?);
            }

            Ok(refs)
        }

        fn get_store_version(&self) -> Result<StoreSchemaVersion, AppError> {
            let conn = self.conn.lock().map_err(|_| {
                AppError::new(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to acquire database lock",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_store_version",
                    },
                )
            })?;

            conn.query_row(
                "SELECT major, minor, applied_at FROM store_version ORDER BY major DESC, minor DESC LIMIT 1",
                [],
                |row| {
                    Ok(StoreSchemaVersion {
                        major: row.get(0)?,
                        minor: row.get(1)?,
                        applied_at: row.get(2)?,
                    })
                },
            )
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to get store version",
                    ErrorContext {
                        component: "run_store",
                        operation: "get_store_version",
                    },
                    source,
                )
            })
        }
    }

    pub fn get_run_detail(
        store: &dyn RunStore,
        run_id: &str,
    ) -> Result<Option<RunDetail>, AppError> {
        let manifest = match store.get_run_manifest(run_id)? {
            Some(m) => m,
            None => return Ok(None),
        };

        let turns = store.list_turns(run_id)?;
        let evidence_matches = store.get_evidence_matches(run_id)?;
        let score_report = store.get_score_report(run_id)?;
        let blob_references = store.get_blob_references(run_id)?;

        Ok(Some(RunDetail {
            manifest,
            turns,
            evidence_matches,
            score_report,
            blob_references,
        }))
    }
}

pub mod migration {
    use super::*;
    use rusqlite::Connection;
    use std::path::Path;

    const CURRENT_MAJOR_VERSION: u32 = 1;
    const CURRENT_MINOR_VERSION: u32 = 0;

    #[derive(Debug, Clone)]
    pub struct Migration {
        pub from_major: u32,
        pub from_minor: u32,
        pub to_major: u32,
        pub to_minor: u32,
        pub migrate: fn(&Connection) -> Result<(), AppError>,
    }

    pub fn get_migrations() -> Vec<Migration> {
        vec![]
    }

    pub fn migrate(store_path: impl AsRef<Path>) -> Result<(), AppError> {
        let conn = Connection::open(store_path.as_ref()).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                "failed to open database for migration",
                ErrorContext {
                    component: "run_store",
                    operation: "migrate",
                },
                source,
            )
        })?;

        let current_version = get_current_version(&conn)?;

        if current_version.major < CURRENT_MAJOR_VERSION
            || (current_version.major == CURRENT_MAJOR_VERSION
                && current_version.minor < CURRENT_MINOR_VERSION)
        {
            run_migrations(&conn, current_version)?;
        }

        Ok(())
    }

    fn get_current_version(conn: &Connection) -> Result<StoreSchemaVersion, AppError> {
        let result = conn.query_row(
            "SELECT major, minor, applied_at FROM store_version ORDER BY major DESC, minor DESC LIMIT 1",
            [],
            |row| {
                Ok(StoreSchemaVersion {
                    major: row.get(0)?,
                    minor: row.get(1)?,
                    applied_at: row.get(2)?,
                })
            },
        );

        match result {
            Ok(version) => Ok(version),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(StoreSchemaVersion {
                major: 0,
                minor: 0,
                applied_at: String::new(),
            }),
            Err(source) => Err(AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                "failed to get current store version",
                ErrorContext {
                    component: "run_store",
                    operation: "get_current_version",
                },
                source,
            )),
        }
    }

    fn run_migrations(conn: &Connection, from: StoreSchemaVersion) -> Result<(), AppError> {
        let migrations = get_migrations();

        let mut current = from;
        for migration in migrations {
            if current.major == migration.from_major && current.minor == migration.from_minor {
                (migration.migrate)(conn)?;

                let now = chrono::Utc::now().to_rfc3339();
                conn.execute(
                    "INSERT INTO store_version (major, minor, applied_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![migration.to_major, migration.to_minor, now],
                )
                .map_err(|source| {
                    AppError::with_source(
                        ErrorCode::PersistenceWriteFailed,
                        "failed to record migration version",
                        ErrorContext {
                            component: "run_store",
                            operation: "run_migrations",
                        },
                        source,
                    )
                })?;

                current = StoreSchemaVersion {
                    major: migration.to_major,
                    minor: migration.to_minor,
                    applied_at: now,
                };
            }
        }

        Ok(())
    }
}

pub use sqlite::SqliteRunStore;
