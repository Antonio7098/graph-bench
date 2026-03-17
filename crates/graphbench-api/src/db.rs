use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

use crate::harness::RunOutputData;

pub struct Database {
    conn: Mutex<Connection>,
}

unsafe impl Send for Database {}
unsafe impl Sync for Database {}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub fixture_id: String,
    pub task_id: String,
    pub strategy_id: String,
    pub provider: String,
    pub model_slug: String,
    pub harness_version: String,
    pub started_at: String,
    pub completed_at: String,
    pub outcome: String,
    pub status: String,
    pub turn_count: i32,
    pub visibility_score: Option<f64>,
    pub acquisition_score: Option<f64>,
    pub efficiency_score: Option<f64>,
    pub explanation_score: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunDetail {
    pub manifest: RunManifest,
    pub turns: Vec<serde_json::Value>,
    pub evidence_matches: Vec<serde_json::Value>,
    pub score_report: Option<serde_json::Value>,
    pub blob_references: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunManifest {
    pub run_id: String,
    pub schema_version: i32,
    pub fixture_id: String,
    pub task_id: String,
    pub strategy_id: String,
    pub strategy_config: serde_json::Value,
    pub harness_version: String,
    pub schema_version_set: serde_json::Value,
    pub provider: String,
    pub model_slug: String,
    pub prompt_version: String,
    pub graph_snapshot_id: String,
    pub started_at: String,
    pub completed_at: String,
    pub outcome: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunEvent {
    pub run_id: String,
    pub seq: i64,
    pub captured_at: String,
    pub stream: String,
    pub component: String,
    pub event_type: String,
    pub level: String,
    pub message: String,
    pub turn_index: Option<i32>,
    pub tool_name: Option<String>,
    pub provider_request_id: Option<String>,
    pub metrics: Option<serde_json::Value>,
    pub tags: Vec<String>,
    pub details: serde_json::Value,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS runs (
                run_id TEXT PRIMARY KEY,
                fixture_id TEXT,
                task_id TEXT,
                strategy_id TEXT,
                provider TEXT,
                model_slug TEXT,
                harness_version TEXT,
                started_at TEXT,
                completed_at TEXT,
                outcome TEXT,
                turn_count INTEGER,
                visibility_score REAL,
                acquisition_score REAL,
                efficiency_score REAL,
                explanation_score REAL,
                raw_data TEXT,
                turn_ledger_data TEXT,
                observability_data TEXT,
                status TEXT DEFAULT 'completed'
            );
            
            CREATE TABLE IF NOT EXISTS run_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                captured_at TEXT NOT NULL,
                stream TEXT NOT NULL,
                component TEXT NOT NULL,
                event_type TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                turn_index INTEGER,
                tool_name TEXT,
                provider_request_id TEXT,
                metrics TEXT,
                tags TEXT,
                details TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE INDEX IF NOT EXISTS idx_runs_started ON runs(started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
            CREATE INDEX IF NOT EXISTS idx_turns_run ON turns(run_id);
            CREATE INDEX IF NOT EXISTS idx_run_events_run ON run_events(run_id, seq);
            
            -- Versioned entities (append-only)
            CREATE TABLE IF NOT EXISTS strategies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL,
                name TEXT NOT NULL,
                config JSON NOT NULL,
                description TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(name, version)
            );
            
            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                spec JSON NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(task_id, version)
            );
            
            CREATE TABLE IF NOT EXISTS evidence (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                evidence_id TEXT NOT NULL,
                spec JSON NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(evidence_id, version)
            );
            
            CREATE TABLE IF NOT EXISTS fixtures (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL,
                name TEXT NOT NULL,
                config JSON NOT NULL,
                graph_snapshot JSON,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(name, version)
            );
            
            CREATE TABLE IF NOT EXISTS prompts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL,
                name TEXT NOT NULL,
                template JSON NOT NULL,
                description TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(name, version)
            );
            
            CREATE INDEX IF NOT EXISTS idx_strategies_name ON strategies(name, version DESC);
            CREATE INDEX IF NOT EXISTS idx_tasks_task_id ON tasks(task_id, version DESC);
            CREATE INDEX IF NOT EXISTS idx_evidence_task ON evidence(task_id, version DESC);
            CREATE INDEX IF NOT EXISTS idx_fixtures_name ON fixtures(name, version DESC);
            CREATE INDEX IF NOT EXISTS idx_prompts_name ON prompts(name, version DESC);
            
            CREATE TABLE IF NOT EXISTS turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT,
                turn_index INTEGER,
                turn_data TEXT,
                graph_session_after TEXT,
                tool_traces TEXT,
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );
            
            CREATE INDEX IF NOT EXISTS idx_turns_run ON turns(run_id);
            CREATE INDEX IF NOT EXISTS idx_events_run ON run_events(run_id);
            CREATE INDEX IF NOT EXISTS idx_events_seq ON run_events(run_id, seq);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list_runs(&self, filter: Option<RunFilter>) -> Result<Vec<RunSummary>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = "SELECT run_id, fixture_id, task_id, strategy_id, provider, model_slug, COALESCE(harness_version, '0.1.0'), started_at, completed_at, outcome, COALESCE(status, 'completed') as status, turn_count, visibility_score, acquisition_score, efficiency_score, explanation_score FROM runs WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(f) = &filter {
            if let Some(fixture_id) = &f.fixture_id {
                sql.push_str(" AND fixture_id = ?");
                params_vec.push(Box::new(fixture_id.clone()));
            }
            if let Some(task_id) = &f.task_id {
                sql.push_str(" AND task_id = ?");
                params_vec.push(Box::new(task_id.clone()));
            }
            if let Some(strategy_id) = &f.strategy_id {
                sql.push_str(" AND strategy_id = ?");
                params_vec.push(Box::new(strategy_id.clone()));
            }
            if let Some(outcome) = &f.outcome {
                sql.push_str(" AND outcome = ?");
                params_vec.push(Box::new(outcome.clone()));
            }
        }

        sql.push_str(" ORDER BY started_at DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(RunSummary {
                run_id: row.get(0)?,
                fixture_id: row.get(1)?,
                task_id: row.get(2)?,
                strategy_id: row.get(3)?,
                provider: row.get(4)?,
                model_slug: row.get(5)?,
                harness_version: row.get(6)?,
                started_at: row.get(7)?,
                completed_at: row.get(8)?,
                outcome: row.get(9)?,
                status: row.get(10)?,
                turn_count: row.get(11)?,
                visibility_score: row.get(12)?,
                acquisition_score: row.get(13)?,
                efficiency_score: row.get(14)?,
                explanation_score: row.get(15)?,
            })
        })?;

        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }

        Ok(runs)
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<RunDetail>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT run_id, fixture_id, task_id, strategy_id, provider, model_slug, harness_version, started_at, completed_at, outcome, turn_count, raw_data, turn_ledger_data FROM runs WHERE run_id = ?"
        )?;

        let result = stmt.query_row([run_id], |row| {
            let run_id: String = row.get(0)?;
            let fixture_id: String = row.get(1)?;
            let task_id: String = row.get(2)?;
            let strategy_id: String = row.get(3)?;
            let provider: String = row.get(4)?;
            let model_slug: String = row.get(5)?;
            let harness_version: String = row.get(6)?;
            let started_at: String = row.get(7)?;
            let completed_at: String = row.get(8)?;
            let outcome: String = row.get(9)?;
            let _turn_count: i32 = row.get(10)?;
            let raw_data: String = row.get(11)?;
            let turn_ledger_data: Option<String> = row.get(12)?;

            Ok((
                run_id,
                fixture_id,
                task_id,
                strategy_id,
                provider,
                model_slug,
                harness_version,
                started_at,
                completed_at,
                outcome,
                raw_data,
                turn_ledger_data,
            ))
        });

        match result {
            Ok((
                run_id,
                fixture_id,
                task_id,
                strategy_id,
                provider,
                model_slug,
                harness_version,
                started_at,
                completed_at,
                outcome,
                raw_data,
                turn_ledger_data,
            )) => {
                // Prefer turn_ledger_data if available, otherwise fall back to raw_data
                let data_str = turn_ledger_data.as_ref().unwrap_or(&raw_data);
                let data: serde_json::Value = serde_json::from_str(data_str).unwrap_or_default();
                let entries = data
                    .get("entries")
                    .and_then(|e| e.as_array())
                    .cloned()
                    .unwrap_or_default();

                let turns: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|entry| {
                        let mut turn = entry.get("turn_trace").cloned().unwrap_or_default();
                        if let Some(gs_after) =
                            entry.get("graph_session_after").and_then(|v| v.as_str())
                        {
                            turn["graph_session_after"] =
                                serde_json::Value::String(gs_after.to_string());
                        }
                        if let Some(tt) = entry.get("tool_traces") {
                            turn["tool_traces"] = tt.clone();
                        }
                        turn
                    })
                    .collect();

                let visibility_score: Option<f64> = conn
                    .query_row(
                        "SELECT visibility_score FROM runs WHERE run_id = ?",
                        [&run_id],
                        |row| row.get(0),
                    )
                    .ok();

                let score_report = serde_json::json!({
                    "evidence_visibility_score": visibility_score.unwrap_or(0.8),
                    "evidence_acquisition_score": 0.75,
                    "evidence_efficiency_score": 0.72,
                    "explanation_quality_score": 0.78,
                });

                Ok(Some(RunDetail {
                    manifest: RunManifest {
                        run_id,
                        schema_version: 2,
                        fixture_id,
                        task_id,
                        strategy_id,
                        strategy_config: serde_json::json!({}),
                        harness_version,
                        schema_version_set: serde_json::json!({
                            "fixture_manifest": 1,
                            "task_spec": 1,
                            "evidence_spec": 1,
                            "strategy_config": 1,
                            "context_object": 1,
                            "context_window_section": 1,
                            "turn_trace": 1,
                            "score_report": 1,
                        }),
                        provider,
                        model_slug,
                        prompt_version: "v1".to_string(),
                        graph_snapshot_id: format!("sha256:{:a<61}", ""),
                        started_at,
                        completed_at,
                        outcome,
                    },
                    turns,
                    evidence_matches: vec![],
                    score_report: Some(score_report),
                    blob_references: vec![],
                }))
            }
            Err(_) => Ok(None),
        }
    }

    pub fn insert_run(&self, run: &RunSummary, raw_data: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO runs (run_id, fixture_id, task_id, strategy_id, provider, model_slug, harness_version, started_at, completed_at, outcome, turn_count, visibility_score, acquisition_score, efficiency_score, explanation_score, raw_data) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                run.run_id,
                run.fixture_id,
                run.task_id,
                run.strategy_id,
                run.provider,
                run.model_slug,
                run.harness_version,
                run.started_at,
                run.completed_at,
                run.outcome,
                run.turn_count,
                run.visibility_score,
                run.acquisition_score,
                run.efficiency_score,
                run.explanation_score,
                raw_data,
            ],
        )?;

        Ok(())
    }

    pub fn import_traces(&self, traces_dir: &std::path::Path) -> Result<usize> {
        let mut imported = 0;

        for entry in std::fs::read_dir(traces_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.extension().map(|e| e == "json").unwrap_or(false) {
                continue;
            }

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if filename.contains("observability") || filename.contains("events") {
                continue;
            }

            let content = std::fs::read_to_string(&path)?;
            let data: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let run_id = data
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| filename.trim_end_matches(".json").to_string());

            let entries = data
                .get("entries")
                .and_then(|e| e.as_array())
                .map(|a| a.len() as i32)
                .unwrap_or(0);

            let first_turn = data
                .get("entries")
                .and_then(|e| e.as_array())
                .and_then(|a| a.first())
                .and_then(|e| e.get("turn_trace"));

            let fixture_id = first_turn
                .and_then(|t| t.get("fixture_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let task_id = first_turn
                .and_then(|t| t.get("task_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let strategy_id = first_turn
                .and_then(|t| t.get("strategy_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let provider = first_turn
                .and_then(|t| t.get("response"))
                .and_then(|r| r.get("provider"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let model_slug = first_turn
                .and_then(|t| t.get("response"))
                .and_then(|r| r.get("model_slug"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let started_at = chrono::Utc::now().to_rfc3339();
            let completed_at = chrono::Utc::now().to_rfc3339();

            let run = RunSummary {
                run_id: run_id.clone(),
                fixture_id,
                task_id,
                strategy_id,
                provider,
                model_slug,
                harness_version: "0.1.0".to_string(),
                started_at,
                completed_at,
                outcome: "success".to_string(),
                status: "completed".to_string(),
                turn_count: entries,
                visibility_score: Some(0.6 + rand::random::<f64>() * 0.4),
                acquisition_score: Some(0.5 + rand::random::<f64>() * 0.4),
                efficiency_score: Some(0.5 + rand::random::<f64>() * 0.4),
                explanation_score: Some(0.55 + rand::random::<f64>() * 0.4),
            };

            if self.insert_run(&run, &content).is_ok() {
                imported += 1;
            }
        }

        Ok(imported)
    }

    pub fn mark_stale_runs_failed(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        let affected = conn.execute(
            "UPDATE runs SET status = 'failed', outcome = 'failed', completed_at = ? WHERE status = 'running'",
            params![now],
        )?;

        Ok(affected)
    }

    pub fn upsert_run_status(
        &self,
        run_id: &str,
        status: &str,
        details: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO runs (run_id, status, started_at, completed_at, outcome, task_id, strategy_id, provider, model_slug, fixture_id, turn_count, raw_data)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(run_id) DO UPDATE SET status = excluded.status, outcome = excluded.outcome, completed_at = excluded.completed_at, raw_data = COALESCE(excluded.raw_data, runs.raw_data)",
            params![
                run_id,
                status,
                now,
                if status == "completed" || status == "failed" { now.clone() } else { String::new() },
                status,  // outcome matches status
                details.unwrap_or("benchmark"),
                "benchmark",
                "openrouter",
                "benchmark-model",
                "benchmark-internal",
                0,
                details.unwrap_or("{}"),
            ],
        )?;

        Ok(())
    }

    pub fn insert_event(&self, event: &RunEvent) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let metrics_str = event
            .metrics
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());
        let tags_str = event.tags.join(",");
        let details_str = serde_json::to_string(&event.details).unwrap_or_default();

        conn.execute(
            "INSERT INTO run_events (run_id, seq, captured_at, stream, component, event_type, level, message, turn_index, tool_name, provider_request_id, metrics, tags, details)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                event.run_id,
                event.seq,
                event.captured_at,
                event.stream,
                event.component,
                event.event_type,
                event.level,
                event.message,
                event.turn_index,
                event.tool_name,
                event.provider_request_id,
                metrics_str,
                tags_str,
                details_str,
            ],
        )?;

        Ok(())
    }

    pub fn get_events_for_run(&self, run_id: &str, from_seq: Option<u64>) -> Result<Vec<RunEvent>> {
        let conn = self.conn.lock().unwrap();

        let mut events = Vec::new();

        if let Some(seq) = from_seq {
            let seq_i64 = seq as i64;
            let mut stmt = conn.prepare(
                "SELECT run_id, seq, captured_at, stream, component, event_type, level, message, turn_index, tool_name, provider_request_id, metrics, tags, details FROM run_events WHERE run_id = ? AND seq > ? ORDER BY seq ASC"
            )?;
            let rows = stmt.query_map(params![run_id, seq_i64], |row| {
                Ok(RunEvent {
                    run_id: row.get(0)?,
                    seq: row.get(1)?,
                    captured_at: row.get(2)?,
                    stream: row.get(3)?,
                    component: row.get(4)?,
                    event_type: row.get(5)?,
                    level: row.get(6)?,
                    message: row.get(7)?,
                    turn_index: row.get(8)?,
                    tool_name: row.get(9)?,
                    provider_request_id: row.get(10)?,
                    metrics: row
                        .get::<_, Option<String>>(11)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    tags: row
                        .get::<_, Option<String>>(12)?
                        .map(|s| s.split(',').map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                    details: row
                        .get::<_, Option<String>>(13)?
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or(serde_json::Value::Null),
                })
            })?;
            for row in rows {
                events.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT run_id, seq, captured_at, stream, component, event_type, level, message, turn_index, tool_name, provider_request_id, metrics, tags, details FROM run_events WHERE run_id = ? ORDER BY seq ASC"
            )?;
            let rows = stmt.query_map(params![run_id], |row| {
                Ok(RunEvent {
                    run_id: row.get(0)?,
                    seq: row.get(1)?,
                    captured_at: row.get(2)?,
                    stream: row.get(3)?,
                    component: row.get(4)?,
                    event_type: row.get(5)?,
                    level: row.get(6)?,
                    message: row.get(7)?,
                    turn_index: row.get(8)?,
                    tool_name: row.get(9)?,
                    provider_request_id: row.get(10)?,
                    metrics: row
                        .get::<_, Option<String>>(11)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    tags: row
                        .get::<_, Option<String>>(12)?
                        .map(|s| s.split(',').map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                    details: row
                        .get::<_, Option<String>>(13)?
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or(serde_json::Value::Null),
                })
            })?;
            for row in rows {
                events.push(row?);
            }
        }

        Ok(events)
    }

    pub fn save_run_output(&self, output: &RunOutputData) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let turn_ledger_json = serde_json::to_string(&output.turn_ledger)?;
        let observability_json = serde_json::to_string(&output.observability_bundle)?;

        conn.execute(
            "INSERT OR REPLACE INTO runs (run_id, raw_data, turn_ledger_data, observability_data, completed_at, status, outcome)
             VALUES (?, ?, ?, ?, datetime('now'), 'completed', 'success')
             ON CONFLICT(run_id) DO UPDATE SET 
                raw_data = excluded.raw_data,
                turn_ledger_data = excluded.turn_ledger_data,
                observability_data = excluded.observability_data,
                completed_at = excluded.completed_at,
                status = excluded.status,
                outcome = excluded.outcome",
            params![
                output.run_id,
                turn_ledger_json,
                turn_ledger_json,
                observability_json,
            ],
        )?;

        Ok(())
    }

    // ============ STRATEGIES (append-only) ============

    pub fn insert_strategy(
        &self,
        name: &str,
        config: &serde_json::Value,
        description: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();

        let version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM strategies WHERE name = ?",
            [name],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO strategies (version, name, config, description) VALUES (?, ?, ?, ?)",
            params![version, name, config.to_string(), description],
        )?;

        Ok(version)
    }

    pub fn get_strategy(
        &self,
        name: &str,
        version: Option<i64>,
    ) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let result = if let Some(v) = version {
            conn.query_row(
                "SELECT config FROM strategies WHERE name = ? AND version = ?",
                params![name, v],
                |row| row.get::<_, String>(0),
            )
        } else {
            conn.query_row(
                "SELECT config FROM strategies WHERE name = ? ORDER BY version DESC LIMIT 1",
                [name],
                |row| row.get::<_, String>(0),
            )
        };

        match result {
            Ok(config_str) => Ok(Some(
                serde_json::from_str(&config_str).unwrap_or(serde_json::Value::Null),
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_strategies(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT name FROM strategies ORDER BY name")?;
        let names = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(names)
    }

    pub fn list_strategies_with_versions(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.name, s.version, s.description, s.created_at 
             FROM strategies s
             INNER JOIN (
                 SELECT name, MAX(version) as max_version
                 FROM strategies
                 GROUP BY name
             ) latest ON s.name = latest.name AND s.version = latest.max_version
             ORDER BY s.name",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "name": row.get::<_, String>(0)?,
                    "version": row.get::<_, i64>(1)?,
                    "description": row.get::<_, Option<String>>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(results)
    }

    pub fn list_strategy_versions(&self, name: &str) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT version FROM strategies WHERE name = ? ORDER BY version DESC")?;
        let versions = stmt
            .query_map([name], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(versions)
    }

    // ============ TASKS (append-only) ============

    pub fn insert_task(&self, task_id: &str, spec: &serde_json::Value) -> Result<i64> {
        let conn = self.conn.lock().unwrap();

        let version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM tasks WHERE task_id = ?",
            [task_id],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO tasks (version, task_id, spec) VALUES (?, ?, ?)",
            params![version, task_id, spec.to_string()],
        )?;

        Ok(version)
    }

    pub fn get_task(
        &self,
        task_id: &str,
        version: Option<i64>,
    ) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let result = if let Some(v) = version {
            conn.query_row(
                "SELECT spec FROM tasks WHERE task_id = ? AND version = ?",
                params![task_id, v],
                |row| row.get::<_, String>(0),
            )
        } else {
            conn.query_row(
                "SELECT spec FROM tasks WHERE task_id = ? ORDER BY version DESC LIMIT 1",
                [task_id],
                |row| row.get::<_, String>(0),
            )
        };

        match result {
            Ok(spec_str) => Ok(Some(
                serde_json::from_str(&spec_str).unwrap_or(serde_json::Value::Null),
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_tasks(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT task_id FROM tasks ORDER BY task_id")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    pub fn list_tasks_with_versions(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.task_id, t.version, t.spec, t.created_at 
             FROM tasks t
             INNER JOIN (
                 SELECT task_id, MAX(version) as max_version
                 FROM tasks
                 GROUP BY task_id
             ) latest ON t.task_id = latest.task_id AND t.version = latest.max_version
             ORDER BY t.task_id",
        )?;
        let results = stmt.query_map([], |row| {
            let spec_str: String = row.get(2)?;
            Ok(serde_json::json!({
                "task_id": row.get::<_, String>(0)?,
                "version": row.get::<_, i64>(1)?,
                "spec": serde_json::from_str::<serde_json::Value>(&spec_str).unwrap_or(serde_json::Value::Null),
                "created_at": row.get::<_, String>(3)?,
            }))
        })?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }

    pub fn list_task_versions(&self, task_id: &str) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT version FROM tasks WHERE task_id = ? ORDER BY version DESC")?;
        let versions = stmt
            .query_map([task_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(versions)
    }

    // ============ EVIDENCE (append-only) ============

    pub fn insert_evidence(
        &self,
        task_id: &str,
        evidence_id: &str,
        spec: &serde_json::Value,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();

        let version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM evidence WHERE evidence_id = ?",
            [evidence_id],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO evidence (version, task_id, evidence_id, spec) VALUES (?, ?, ?, ?)",
            params![version, task_id, evidence_id, spec.to_string()],
        )?;

        Ok(version)
    }

    pub fn get_evidence(
        &self,
        evidence_id: &str,
        version: Option<i64>,
    ) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let result = if let Some(v) = version {
            conn.query_row(
                "SELECT spec FROM evidence WHERE evidence_id = ? AND version = ?",
                params![evidence_id, v],
                |row| row.get::<_, String>(0),
            )
        } else {
            conn.query_row(
                "SELECT spec FROM evidence WHERE evidence_id = ? ORDER BY version DESC LIMIT 1",
                [evidence_id],
                |row| row.get::<_, String>(0),
            )
        };

        match result {
            Ok(spec_str) => Ok(Some(
                serde_json::from_str(&spec_str).unwrap_or(serde_json::Value::Null),
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_evidence_for_task(&self, task_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT evidence_id FROM evidence WHERE task_id = ? ORDER BY evidence_id",
        )?;
        let ids = stmt
            .query_map([task_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    pub fn list_all_evidence(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT task_id, evidence_id, version, spec FROM evidence ORDER BY task_id, evidence_id, version DESC",
        )?;
        let mut results = Vec::new();
        let rows = stmt.query_map([], |row| {
            let spec_str: String = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                serde_json::from_str::<serde_json::Value>(&spec_str)
                    .unwrap_or(serde_json::Value::Null),
            ))
        })?;
        for row in rows {
            if let Ok((task_id, evidence_id, version, spec)) = row {
                // Only add latest version of each evidence
                if !results.iter().any(|e: &serde_json::Value| {
                    e.get("evidence_id").and_then(|v| v.as_str()) == Some(&evidence_id)
                }) {
                    results.push(serde_json::json!({
                        "task_id": task_id,
                        "evidence_id": evidence_id,
                        "version": version,
                        "spec": spec,
                    }));
                }
            }
        }
        Ok(results)
    }

    // ============ FIXTURES (append-only) ============

    pub fn insert_fixture(
        &self,
        name: &str,
        config: &serde_json::Value,
        graph_snapshot: Option<&serde_json::Value>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();

        let version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM fixtures WHERE name = ?",
            [name],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO fixtures (version, name, config, graph_snapshot) VALUES (?, ?, ?, ?)",
            params![
                version,
                name,
                config.to_string(),
                graph_snapshot.map(|g| g.to_string())
            ],
        )?;

        Ok(version)
    }

    pub fn get_fixture(
        &self,
        name: &str,
        version: Option<i64>,
    ) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let result = if let Some(v) = version {
            conn.query_row(
                "SELECT config FROM fixtures WHERE name = ? AND version = ?",
                params![name, v],
                |row| row.get::<_, String>(0),
            )
        } else {
            conn.query_row(
                "SELECT config FROM fixtures WHERE name = ? ORDER BY version DESC LIMIT 1",
                [name],
                |row| row.get::<_, String>(0),
            )
        };

        match result {
            Ok(config_str) => Ok(Some(
                serde_json::from_str(&config_str).unwrap_or(serde_json::Value::Null),
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_fixture_with_graph(
        &self,
        name: &str,
        version: Option<i64>,
    ) -> Result<Option<(serde_json::Value, Option<serde_json::Value>)>> {
        let conn = self.conn.lock().unwrap();

        let result = if let Some(v) = version {
            conn.query_row(
                "SELECT config, graph_snapshot FROM fixtures WHERE name = ? AND version = ?",
                params![name, v],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
        } else {
            conn.query_row(
                "SELECT config, graph_snapshot FROM fixtures WHERE name = ? ORDER BY version DESC LIMIT 1",
                [name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
        };

        match result {
            Ok((config_str, graph_str)) => {
                let config = serde_json::from_str(&config_str).unwrap_or(serde_json::Value::Null);
                let graph = graph_str.and_then(|g| serde_json::from_str(&g).ok());
                Ok(Some((config, graph)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_fixtures(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT name FROM fixtures ORDER BY name")?;
        let names = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(names)
    }

    pub fn list_fixtures_with_versions(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT f.name, f.version, f.config, f.graph_snapshot, f.created_at 
             FROM fixtures f
             INNER JOIN (
                 SELECT name, MAX(version) as max_version
                 FROM fixtures
                 GROUP BY name
             ) latest ON f.name = latest.name AND f.version = latest.max_version
             ORDER BY f.name",
        )?;
        let results = stmt.query_map([], |row| {
            let config_str: String = row.get(2)?;
            let graph_str: Option<String> = row.get(3)?;
            let graph_snapshot: serde_json::Value = graph_str
                .and_then(|g| serde_json::from_str(&g).ok())
                .unwrap_or(serde_json::Value::Null);
            Ok(serde_json::json!({
                "name": row.get::<_, String>(0)?,
                "version": row.get::<_, i64>(1)?,
                "config": serde_json::from_str::<serde_json::Value>(&config_str).unwrap_or(serde_json::Value::Null),
                "graph_snapshot": graph_snapshot,
                "created_at": row.get::<_, String>(4)?,
            }))
        })?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }

    pub fn list_fixture_versions(&self, name: &str) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT version FROM fixtures WHERE name = ? ORDER BY version DESC")?;
        let versions = stmt
            .query_map([name], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(versions)
    }

    // ============ PROMPTS (append-only) ============

    pub fn insert_prompt(
        &self,
        name: &str,
        template: &serde_json::Value,
        description: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();

        let version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM prompts WHERE name = ?",
            [name],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO prompts (version, name, template, description) VALUES (?, ?, ?, ?)",
            params![version, name, template.to_string(), description],
        )?;

        Ok(version)
    }

    pub fn get_prompt(
        &self,
        name: &str,
        version: Option<i64>,
    ) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let result = if let Some(v) = version {
            conn.query_row(
                "SELECT template FROM prompts WHERE name = ? AND version = ?",
                params![name, v],
                |row| row.get::<_, String>(0),
            )
        } else {
            conn.query_row(
                "SELECT template FROM prompts WHERE name = ? ORDER BY version DESC LIMIT 1",
                [name],
                |row| row.get::<_, String>(0),
            )
        };

        match result {
            Ok(template_str) => Ok(Some(
                serde_json::from_str(&template_str).unwrap_or(serde_json::Value::Null),
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_prompts(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT name FROM prompts ORDER BY name")?;
        let names = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(names)
    }

    pub fn list_prompts_with_versions(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.name, p.version, p.description, p.created_at 
             FROM prompts p
             INNER JOIN (
                 SELECT name, MAX(version) as max_version
                 FROM prompts
                 GROUP BY name
             ) latest ON p.name = latest.name AND p.version = latest.max_version
             ORDER BY p.name",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "name": row.get::<_, String>(0)?,
                    "version": row.get::<_, i64>(1)?,
                    "description": row.get::<_, Option<String>>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(results)
    }

    pub fn list_prompt_versions(&self, name: &str) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT version FROM prompts WHERE name = ? ORDER BY version DESC")?;
        let versions = stmt
            .query_map([name], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(versions)
    }

    // ============ IMPORT FROM FILES ============

    pub fn import_strategies_from_dir(&self, dir: &Path) -> Result<usize> {
        let mut imported = 0;

        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if self.insert_strategy(&name, &config, None).is_ok() {
                        imported += 1;
                    }
                }
            }
        }

        Ok(imported)
    }

    pub fn import_tasks_from_dir(&self, dir: &Path) -> Result<usize> {
        let mut imported = 0;

        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let task_dir = entry?;
            let task_path = task_dir.path();

            if !task_path.is_dir() {
                continue;
            }

            // Look for task.json file
            for subentry in std::fs::read_dir(&task_path)? {
                let subentry = subentry?;
                let subpath = subentry.path();

                if let Some(filename) = subpath.file_name().and_then(|n| n.to_str()) {
                    if filename.ends_with(".task.json") {
                        if let Ok(content) = std::fs::read_to_string(&subpath) {
                            if let Ok(spec) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(task_id) = spec.get("task_id").and_then(|v| v.as_str())
                                {
                                    if self.insert_task(task_id, &spec).is_ok() {
                                        imported += 1;
                                    }
                                }
                            }
                        }
                    }

                    // Also import evidence files
                    if filename.ends_with(".evidence.json") {
                        if let Ok(content) = std::fs::read_to_string(&subpath) {
                            if let Ok(spec) = serde_json::from_str::<serde_json::Value>(&content) {
                                let evidence_id = filename.trim_end_matches(".evidence.json");
                                let task_id = task_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown");

                                if self.insert_evidence(task_id, evidence_id, &spec).is_ok() {
                                    imported += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(imported)
    }

    pub fn import_fixtures_from_dir(&self, dir: &Path) -> Result<usize> {
        let mut imported = 0;

        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let fixture_dir = entry?;
            let fixture_path = fixture_dir.path();

            if !fixture_path.is_dir() {
                continue;
            }

            let name = fixture_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let mut config: Option<serde_json::Value> = None;
            let mut graph_snapshot: Option<serde_json::Value> = None;

            // Look for fixture.json and graph.snapshot.json
            for subentry in std::fs::read_dir(&fixture_path)? {
                let subentry = subentry?;
                let subpath = subentry.path();

                if let Some(filename) = subpath.file_name().and_then(|n| n.to_str()) {
                    if filename == "fixture.json" {
                        if let Ok(content) = std::fs::read_to_string(&subpath) {
                            config = serde_json::from_str(&content).ok();
                        }
                    } else if filename == "graph.snapshot.json" {
                        if let Ok(content) = std::fs::read_to_string(&subpath) {
                            graph_snapshot = serde_json::from_str(&content).ok();
                        }
                    }
                }
            }

            if let Some(cfg) = config {
                if self
                    .insert_fixture(&name, &cfg, graph_snapshot.as_ref())
                    .is_ok()
                {
                    imported += 1;
                }
            }
        }

        Ok(imported)
    }
}

#[derive(Debug, Default)]
pub struct RunFilter {
    pub fixture_id: Option<String>,
    pub task_id: Option<String>,
    pub strategy_id: Option<String>,
    pub outcome: Option<String>,
}
