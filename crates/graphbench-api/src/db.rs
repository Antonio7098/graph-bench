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
                status TEXT DEFAULT 'completed'
            );

            CREATE TABLE IF NOT EXISTS run_manifests (
                run_id TEXT PRIMARY KEY,
                schema_version INTEGER,
                harness_version TEXT,
                provider TEXT,
                model_slug TEXT,
                prompt_version TEXT,
                graph_snapshot_id TEXT,
                started_at TEXT,
                completed_at TEXT,
                outcome TEXT,
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_telemetry_aggregations (
                run_id TEXT PRIMARY KEY,
                total_turns INTEGER,
                aggregate_prompt_bytes INTEGER,
                aggregate_prompt_tokens INTEGER,
                aggregate_latency_ms INTEGER,
                aggregate_tool_calls INTEGER,
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_states (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                state_type TEXT NOT NULL,
                state_value TEXT NOT NULL,
                PRIMARY KEY (run_id, turn_index, state_type),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_requests (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                schema_version INTEGER,
                prompt_version TEXT,
                prompt_hash TEXT,
                context_hash TEXT,
                PRIMARY KEY (run_id, turn_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_responses (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                provider TEXT,
                model_slug TEXT,
                schema_version INTEGER,
                validated INTEGER,
                PRIMARY KEY (run_id, turn_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_selections (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                selected_context_object TEXT,
                PRIMARY KEY (run_id, turn_index, selected_context_object),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_omitted_candidates (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                candidate_index INTEGER NOT NULL,
                candidate_id TEXT,
                reason TEXT,
                PRIMARY KEY (run_id, turn_index, candidate_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_rendered_sections (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                section_index INTEGER NOT NULL,
                section_id TEXT,
                schema_version INTEGER,
                title TEXT,
                content TEXT,
                byte_count INTEGER,
                token_count INTEGER,
                PRIMARY KEY (run_id, turn_index, section_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_context_ids (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                context_index INTEGER NOT NULL,
                context_object_id TEXT,
                PRIMARY KEY (run_id, turn_index, context_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_compactions (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                compaction_index INTEGER NOT NULL,
                summary_item_id TEXT,
                source_item_id TEXT,
                PRIMARY KEY (run_id, turn_index, compaction_index, source_item_id),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_section_accounting (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                section_id TEXT,
                byte_count INTEGER,
                token_count INTEGER,
                PRIMARY KEY (run_id, turn_index, section_id),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_telemetry (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                prompt_bytes INTEGER,
                prompt_tokens INTEGER,
                latency_ms INTEGER,
                tool_calls INTEGER,
                PRIMARY KEY (run_id, turn_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_hashes (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                turn_hash TEXT,
                PRIMARY KEY (run_id, turn_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_readiness (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                readiness_state TEXT,
                readiness_reason TEXT,
                PRIMARY KEY (run_id, turn_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_evidence_delta (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                evidence_index INTEGER NOT NULL,
                evidence_id TEXT,
                PRIMARY KEY (run_id, turn_index, evidence_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turns (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                graph_session_before TEXT,
                graph_session_after TEXT,
                rendered_prompt TEXT,
                rendered_context TEXT,
                replay_hash TEXT,
                provider_request_id TEXT,
                PRIMARY KEY (run_id, turn_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_tool_traces (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                trace_index INTEGER NOT NULL,
                tool_name TEXT,
                latency_ms INTEGER,
                outcome TEXT,
                input_payload_json TEXT,
                output_payload_json TEXT,
                PRIMARY KEY (run_id, turn_index, trace_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS run_turn_payloads (
                run_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                payload_type TEXT NOT NULL,
                blob_id TEXT,
                media_type TEXT,
                byte_count INTEGER,
                inline_content TEXT,
                PRIMARY KEY (run_id, turn_index, payload_type),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
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
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS run_event_metrics (
                event_id INTEGER PRIMARY KEY,
                run_id TEXT NOT NULL,
                prompt_tokens INTEGER,
                completion_tokens INTEGER,
                total_tokens INTEGER,
                latency_ms INTEGER,
                FOREIGN KEY (run_id) REFERENCES runs(run_id),
                FOREIGN KEY (event_id) REFERENCES run_events(id)
            );

            CREATE TABLE IF NOT EXISTS run_event_tags (
                event_id INTEGER PRIMARY KEY,
                run_id TEXT NOT NULL,
                tag_index INTEGER NOT NULL,
                tag TEXT,
                FOREIGN KEY (run_id) REFERENCES runs(run_id),
                FOREIGN KEY (event_id) REFERENCES run_events(id)
            );

            CREATE TABLE IF NOT EXISTS run_event_details_data (
                event_id INTEGER PRIMARY KEY,
                run_id TEXT NOT NULL,
                details_key TEXT,
                details_value TEXT,
                FOREIGN KEY (run_id) REFERENCES runs(run_id),
                FOREIGN KEY (event_id) REFERENCES run_events(id)
            );

            CREATE TABLE IF NOT EXISTS run_structured_logs (
                run_id TEXT NOT NULL,
                log_index INTEGER NOT NULL,
                log_level TEXT,
                log_component TEXT,
                log_message TEXT,
                log_timestamp TEXT,
                turn_index INTEGER,
                tool_name TEXT,
                PRIMARY KEY (run_id, log_index),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE INDEX IF NOT EXISTS idx_runs_started ON runs(started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
            CREATE INDEX IF NOT EXISTS idx_run_events_run ON run_events(run_id, seq);
            CREATE INDEX IF NOT EXISTS idx_run_turns_run ON run_turns(run_id);
            CREATE INDEX IF NOT EXISTS idx_run_turn_tool_traces_run ON run_turn_tool_traces(run_id, turn_index);
            CREATE INDEX IF NOT EXISTS idx_run_structured_logs_run ON run_structured_logs(run_id);
            
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

        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE run_id = ?)",
                [run_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !exists {
            return Ok(None);
        }

        let mut run_stmt = conn.prepare(
            "SELECT run_id, fixture_id, task_id, strategy_id, provider, model_slug, harness_version, started_at, completed_at, outcome, turn_count FROM runs WHERE run_id = ?"
        )?;

        let run_id_param = run_id.to_string();
        let (
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
            _turn_count,
        ): (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            i32,
        ) = run_stmt
            .query_row([&run_id_param], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            })
            .map_err(|e| rusqlite::Error::QueryReturnedNoRows)?;

        let manifest: RunManifest = if let Ok(row) = conn.query_row(
            "SELECT schema_version, harness_version, provider, model_slug, prompt_version, graph_snapshot_id, started_at, completed_at, outcome FROM run_manifests WHERE run_id = ?",
            [&run_id],
            |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        ) {
            RunManifest {
                run_id: run_id.clone(),
                schema_version: row.0,
                fixture_id: fixture_id.clone(),
                task_id: task_id.clone(),
                strategy_id: strategy_id.clone(),
                strategy_config: serde_json::json!({}),
                harness_version: row.1,
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
                provider: row.2,
                model_slug: row.3,
                prompt_version: row.4,
                graph_snapshot_id: row.5,
                started_at: row.6,
                completed_at: row.7,
                outcome: row.8,
            }
        } else {
            RunManifest {
                run_id: run_id.clone(),
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
                graph_snapshot_id: "sha256:".to_string(),
                started_at,
                completed_at,
                outcome,
            }
        };

        let mut turns_stmt = conn.prepare(
            "SELECT turn_index, graph_session_before, graph_session_after, rendered_prompt, rendered_context, replay_hash, provider_request_id FROM run_turns WHERE run_id = ? ORDER BY turn_index"
        )?;

        let mut turns: Vec<serde_json::Value> = Vec::new();
        let run_id_clone = run_id.clone();
        let mut turn_rows = turns_stmt.query([&run_id_clone])?;

        while let Some(row) = turn_rows.next()? {
            let turn_index: u32 = row.get(0)?;
            let graph_session_before: String = row.get(1)?;
            let graph_session_after: String = row.get(2)?;
            let rendered_prompt: String = row.get(3)?;
            let rendered_context: String = row.get(4)?;
            let replay_hash: String = row.get(5)?;
            let provider_request_id: Option<String> = row.get(6)?;

            // Get state before/after
            let mut state_stmt = conn.prepare(
                "SELECT state_type, state_value FROM run_turn_states WHERE run_id = ? AND turn_index = ?"
            )?;
            let mut states: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut state_rows = state_stmt.query(params![&run_id_clone, turn_index])?;
            while let Some(state_row) = state_rows.next()? {
                let state_type: String = state_row.get(0)?;
                let state_value: String = state_row.get(1)?;
                states.insert(state_type, state_value);
            }

            // Get request
            let mut req_stmt = conn.prepare(
                "SELECT schema_version, prompt_version, prompt_hash, context_hash FROM run_turn_requests WHERE run_id = ? AND turn_index = ?"
            )?;
            let request: serde_json::Value = if let Ok(row) =
                req_stmt.query_row(params![&run_id_clone, turn_index], |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                }) {
                serde_json::json!({
                    "schema_version": row.0,
                    "prompt_version": row.1,
                    "prompt_hash": row.2,
                    "context_hash": row.3,
                })
            } else {
                serde_json::json!({})
            };

            // Get response
            let mut resp_stmt = conn.prepare(
                "SELECT provider, model_slug, schema_version, validated FROM run_turn_responses WHERE run_id = ? AND turn_index = ?"
            )?;
            let response: serde_json::Value = if let Ok(row) =
                resp_stmt.query_row(params![&run_id_clone, turn_index], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i32>(2)?,
                        row.get::<_, i32>(3)?,
                    ))
                }) {
                serde_json::json!({
                    "provider": row.0,
                    "model_slug": row.1,
                    "schema_version": row.2,
                    "validated": row.3 != 0,
                })
            } else {
                serde_json::json!({})
            };

            // Get telemetry
            let mut tel_stmt = conn.prepare(
                "SELECT prompt_bytes, prompt_tokens, latency_ms, tool_calls FROM run_turn_telemetry WHERE run_id = ? AND turn_index = ?"
            )?;
            let telemetry: serde_json::Value = if let Ok(row) =
                tel_stmt.query_row(params![&run_id_clone, turn_index], |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                        row.get::<_, i32>(3)?,
                    ))
                }) {
                serde_json::json!({
                    "prompt_bytes": row.0,
                    "prompt_tokens": row.1,
                    "latency_ms": row.2,
                    "tool_calls": row.3,
                })
            } else {
                serde_json::json!({"prompt_bytes": 0, "prompt_tokens": 0, "latency_ms": 0, "tool_calls": 0})
            };

            // Get hashes
            let mut hash_stmt = conn.prepare(
                "SELECT turn_hash FROM run_turn_hashes WHERE run_id = ? AND turn_index = ?",
            )?;
            let turn_hash: String = hash_stmt
                .query_row(params![&run_id_clone, turn_index], |row| row.get(0))
                .unwrap_or_default();

            // Get readiness
            let mut readiness_stmt = conn.prepare(
                "SELECT readiness_state, readiness_reason FROM run_turn_readiness WHERE run_id = ? AND turn_index = ?"
            )?;
            let (readiness_state, readiness_reason): (String, String) = readiness_stmt
                .query_row(params![&run_id_clone, turn_index], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .unwrap_or(("Unknown".to_string(), "".to_string()));

            // Get selection - context objects
            let mut sel_stmt = conn.prepare(
                "SELECT selected_context_object FROM run_turn_selections WHERE run_id = ? AND turn_index = ?"
            )?;
            let mut selected_context_objects: Vec<String> = Vec::new();
            let mut sel_rows = sel_stmt.query(params![&run_id_clone, turn_index])?;
            while let Some(sel_row) = sel_rows.next()? {
                selected_context_objects.push(sel_row.get(0)?);
            }

            // Get omitted candidates
            let mut om_stmt = conn.prepare(
                "SELECT candidate_id, reason FROM run_turn_omitted_candidates WHERE run_id = ? AND turn_index = ? ORDER BY candidate_index"
            )?;
            let mut omitted_candidates: Vec<serde_json::Value> = Vec::new();
            let mut om_rows = om_stmt.query(params![&run_id_clone, turn_index])?;
            while let Some(om_row) = om_rows.next()? {
                omitted_candidates.push(serde_json::json!({
                    "candidate_id": om_row.get::<_, String>(0)?,
                    "reason": om_row.get::<_, String>(1)?,
                }));
            }

            // Get rendered sections
            let mut sec_stmt = conn.prepare(
                "SELECT section_id, schema_version, title, content, byte_count, token_count FROM run_turn_rendered_sections WHERE run_id = ? AND turn_index = ? ORDER BY section_index"
            )?;
            let mut rendered_sections: Vec<serde_json::Value> = Vec::new();
            let mut sec_rows = sec_stmt.query(params![&run_id_clone, turn_index])?;
            while let Some(sec_row) = sec_rows.next()? {
                rendered_sections.push(serde_json::json!({
                    "section_id": sec_row.get::<_, String>(0)?,
                    "schema_version": sec_row.get::<_, i32>(1)?,
                    "title": sec_row.get::<_, String>(2)?,
                    "content": sec_row.get::<_, String>(3)?,
                    "byte_count": sec_row.get::<_, i32>(4)?,
                    "token_count": sec_row.get::<_, i32>(5)?,
                }));
            }

            // Get evidence delta
            let mut ev_stmt = conn.prepare(
                "SELECT evidence_id FROM run_turn_evidence_delta WHERE run_id = ? AND turn_index = ? ORDER BY evidence_index"
            )?;
            let mut evidence_delta: Vec<String> = Vec::new();
            let mut ev_rows = ev_stmt.query(params![&run_id_clone, turn_index])?;
            while let Some(ev_row) = ev_rows.next()? {
                evidence_delta.push(ev_row.get(0)?);
            }

            // Get tool traces
            let mut tool_traces_stmt = conn.prepare(
                "SELECT trace_index, tool_name, latency_ms, outcome, input_payload_json, output_payload_json FROM run_turn_tool_traces WHERE run_id = ? AND turn_index = ? ORDER BY trace_index"
            )?;

            let mut tool_traces: Vec<serde_json::Value> = Vec::new();
            let mut trace_rows = tool_traces_stmt.query(params![&run_id_clone, turn_index])?;

            while let Some(trace_row) = trace_rows.next()? {
                let tool_name: String = trace_row.get(1)?;
                let latency_ms: u32 = trace_row.get(2)?;
                let outcome: String = trace_row.get(3)?;
                let input_payload_json: String = trace_row.get(4)?;
                let output_payload_json: String = trace_row.get(5)?;

                tool_traces.push(serde_json::json!({
                    "tool_name": tool_name,
                    "latency_ms": latency_ms,
                    "outcome": outcome,
                    "input_payload": serde_json::from_str(&input_payload_json).unwrap_or(serde_json::Value::Null),
                    "output_payload": serde_json::from_str(&output_payload_json).unwrap_or(serde_json::Value::Null),
                }));
            }

            // Get blobs
            let mut payload_stmt = conn.prepare(
                "SELECT payload_type, blob_id, media_type, byte_count, inline_content FROM run_turn_payloads WHERE run_id = ? AND turn_index = ?"
            )?;

            let mut blob_references: Vec<serde_json::Value> = Vec::new();
            let mut payload_rows = payload_stmt.query(params![&run_id_clone, turn_index])?;

            while let Some(payload_row) = payload_rows.next()? {
                let payload_type: String = payload_row.get(0)?;
                let blob_id: String = payload_row.get(1)?;
                let media_type: String = payload_row.get(2)?;
                let byte_count: i64 = payload_row.get(3)?;
                let inline_content: Option<String> = payload_row.get(4)?;

                blob_references.push(serde_json::json!({
                    "blob_id": blob_id,
                    "media_type": media_type,
                    "payload_type": payload_type,
                    "byte_count": byte_count,
                    "inline_content": inline_content,
                }));
            }

            // Construct turn_trace
            let turn_trace = serde_json::json!({
                "run_id": run_id_clone,
                "turn_index": turn_index,
                "task_id": "",
                "fixture_id": "",
                "strategy_id": "",
                "request": request,
                "response": response,
                "selection": {
                    "selected_context_objects": selected_context_objects,
                    "omitted_candidates": omitted_candidates,
                    "rendered_sections": rendered_sections,
                },
                "telemetry": telemetry,
                "evidence_delta": evidence_delta,
                "readiness_state": readiness_state,
                "readiness_reason": readiness_reason,
                "hashes": {
                    "turn_hash": turn_hash,
                },
                "graph_session_before": graph_session_before,
                "graph_session_after": graph_session_after,
            });

            turns.push(serde_json::json!({
                "turn_index": turn_index,
                "rendered_prompt": rendered_prompt,
                "rendered_context": rendered_context,
                "provider_request_id": provider_request_id,
                "telemetry": telemetry,
                "tool_traces": tool_traces,
                "blob_references": blob_references,
                "turn_trace": turn_trace,
            }));
        }

        let visibility_score: Option<f64> = conn
            .query_row(
                "SELECT visibility_score FROM runs WHERE run_id = ?",
                [&run_id_clone],
                |row| row.get(0),
            )
            .ok();

        let acquisition_score: Option<f64> = conn
            .query_row(
                "SELECT acquisition_score FROM runs WHERE run_id = ?",
                [&run_id_clone],
                |row| row.get(0),
            )
            .ok();

        let efficiency_score: Option<f64> = conn
            .query_row(
                "SELECT efficiency_score FROM runs WHERE run_id = ?",
                [&run_id_clone],
                |row| row.get(0),
            )
            .ok();

        let explanation_score: Option<f64> = conn
            .query_row(
                "SELECT explanation_score FROM runs WHERE run_id = ?",
                [&run_id_clone],
                |row| row.get(0),
            )
            .ok();

        let score_report = serde_json::json!({
            "evidence_visibility_score": visibility_score.unwrap_or(0.8),
            "evidence_acquisition_score": acquisition_score.unwrap_or(0.75),
            "evidence_efficiency_score": efficiency_score.unwrap_or(0.72),
            "explanation_quality_score": explanation_score.unwrap_or(0.78),
        });

        Ok(Some(RunDetail {
            manifest,
            turns,
            evidence_matches: vec![],
            score_report: Some(score_report),
            blob_references: vec![],
        }))
    }

    pub fn insert_run(&self, run: &RunSummary) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO runs (run_id, fixture_id, task_id, strategy_id, provider, model_slug, harness_version, started_at, completed_at, outcome, turn_count, visibility_score, acquisition_score, efficiency_score, explanation_score) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
            ],
        )?;

        Ok(())
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
        task_id: Option<&str>,
        strategy_id: Option<&str>,
        fixture_id: Option<&str>,
        model_slug: Option<&str>,
    ) -> Result<()> {
        self.upsert_run_status_with_turns(
            run_id,
            status,
            details,
            task_id,
            strategy_id,
            fixture_id,
            model_slug,
            0,
        )
    }

    pub fn upsert_run_status_with_turns(
        &self,
        run_id: &str,
        status: &str,
        _details: Option<&str>,
        task_id: Option<&str>,
        strategy_id: Option<&str>,
        fixture_id: Option<&str>,
        model_slug: Option<&str>,
        turn_count: i32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO runs (run_id, status, started_at, completed_at, outcome, task_id, strategy_id, provider, model_slug, fixture_id, turn_count)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(run_id) DO UPDATE SET status = excluded.status, outcome = excluded.outcome, completed_at = excluded.completed_at, turn_count = excluded.turn_count",
            params![
                run_id,
                status,
                now,
                if status == "completed" || status == "failed" { now.clone() } else { String::new() },
                status,
                task_id.unwrap_or("benchmark"),
                strategy_id.unwrap_or("benchmark"),
                "openrouter",
                model_slug.unwrap_or("benchmark-model"),
                fixture_id.unwrap_or("benchmark-internal"),
                turn_count,
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
        let bundle = &output.observability_bundle;
        let manifest = &bundle.run_manifest;
        let turn_ledger = &output.turn_ledger;

        conn.execute(
            "INSERT OR REPLACE INTO runs (run_id, completed_at, status, outcome)
             VALUES (?, datetime('now'), 'completed', 'success')
             ON CONFLICT(run_id) DO UPDATE SET 
                completed_at = excluded.completed_at,
                status = excluded.status,
                outcome = excluded.outcome",
            params![output.run_id],
        )?;

        conn.execute(
            "INSERT OR REPLACE INTO run_manifests 
             (run_id, schema_version, harness_version, provider, model_slug, prompt_version, graph_snapshot_id, started_at, completed_at, outcome)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                output.run_id,
                manifest.schema_version,
                manifest.harness_version,
                manifest.provider,
                manifest.model_slug,
                manifest.prompt_version,
                manifest.graph_snapshot_id,
                manifest.started_at,
                manifest.completed_at,
                manifest.outcome,
            ],
        )?;

        conn.execute(
            "INSERT OR REPLACE INTO run_telemetry_aggregations 
             (run_id, total_turns, aggregate_prompt_bytes, aggregate_prompt_tokens, aggregate_latency_ms, aggregate_tool_calls)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                output.run_id,
                bundle.aggregation.total_turns as i64,
                bundle.aggregation.aggregate_prompt_bytes as i64,
                bundle.aggregation.aggregate_prompt_tokens as i64,
                bundle.aggregation.aggregate_latency_ms as i64,
                bundle.aggregation.aggregate_tool_calls as i64,
            ],
        )?;

        // Save turn data from TurnLedger
        for (turn_idx, entry) in turn_ledger.entries.iter().enumerate() {
            let turn_index = entry.turn_trace.turn_index;

            let telemetry_json =
                serde_json::to_string(&entry.turn_trace.telemetry).unwrap_or_default();

            // Basic turn data
            conn.execute(
                "INSERT OR REPLACE INTO run_turns 
                 (run_id, turn_index, graph_session_before, graph_session_after, rendered_prompt, rendered_context, replay_hash, telemetry_json)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    output.run_id,
                    turn_index,
                    entry.graph_session_before,
                    entry.graph_session_after,
                    entry.rendered_prompt,
                    entry.rendered_context,
                    entry.replay_hash,
                    telemetry_json,
                ],
            )?;

            // State before/after
            conn.execute(
                "INSERT OR REPLACE INTO run_turn_states 
                 (run_id, turn_index, state_type, state_value)
                 VALUES (?, ?, 'before', ?)",
                params![
                    output.run_id,
                    turn_index,
                    format!("{:?}", entry.state_before)
                ],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO run_turn_states 
                 (run_id, turn_index, state_type, state_value)
                 VALUES (?, ?, 'after', ?)",
                params![
                    output.run_id,
                    turn_index,
                    format!("{:?}", entry.state_after)
                ],
            )?;

            // Request
            conn.execute(
                "INSERT OR REPLACE INTO run_turn_requests 
                 (run_id, turn_index, schema_version, prompt_version, prompt_hash, context_hash)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    output.run_id,
                    turn_index,
                    entry.turn_trace.request.schema_version as i32,
                    entry.turn_trace.request.prompt_version,
                    entry.turn_trace.request.prompt_hash,
                    entry.turn_trace.request.context_hash,
                ],
            )?;

            // Response
            conn.execute(
                "INSERT OR REPLACE INTO run_turn_responses 
                 (run_id, turn_index, provider, model_slug, schema_version, validated)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    output.run_id,
                    turn_index,
                    entry.turn_trace.response.provider,
                    entry.turn_trace.response.model_slug,
                    entry.turn_trace.response.schema_version as i32,
                    entry.turn_trace.response.validated as i32,
                ],
            )?;

            // Selection - context objects
            for (ctx_idx, ctx_id) in entry
                .turn_trace
                .selection
                .selected_context_objects
                .iter()
                .enumerate()
            {
                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_selections 
                     (run_id, turn_index, selected_context_object)
                     VALUES (?, ?, ?)",
                    params![output.run_id, turn_index, ctx_id],
                )?;
            }

            // Omitted candidates
            for (om_idx, om_cand) in entry
                .turn_trace
                .selection
                .omitted_candidates
                .iter()
                .enumerate()
            {
                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_omitted_candidates 
                     (run_id, turn_index, candidate_index, candidate_id, reason)
                     VALUES (?, ?, ?, ?, ?)",
                    params![
                        output.run_id,
                        turn_index,
                        om_idx as i32,
                        om_cand.candidate_id,
                        om_cand.reason
                    ],
                )?;
            }

            // Rendered sections
            for (sec_idx, section) in entry
                .turn_trace
                .selection
                .rendered_sections
                .iter()
                .enumerate()
            {
                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_rendered_sections 
                     (run_id, turn_index, section_index, section_id, schema_version, title, content, byte_count, token_count)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        output.run_id,
                        turn_index,
                        sec_idx as i32,
                        section.section_id,
                        section.schema_version as i32,
                        section.title,
                        section.content,
                        section.byte_count as i32,
                        section.token_count as i32,
                    ],
                )?;
            }

            // Context object IDs
            for (ctx_idx, ctx_id) in entry.ordered_context_object_ids.iter().enumerate() {
                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_context_ids 
                     (run_id, turn_index, context_index, context_object_id)
                     VALUES (?, ?, ?, ?)",
                    params![output.run_id, turn_index, ctx_idx as i32, ctx_id],
                )?;
            }

            // Compactions
            for (comp_idx, comp) in entry.compactions.iter().enumerate() {
                for src_id in &comp.source_item_ids {
                    conn.execute(
                        "INSERT OR REPLACE INTO run_turn_compactions 
                         (run_id, turn_index, compaction_index, summary_item_id, source_item_id)
                         VALUES (?, ?, ?, ?, ?)",
                        params![
                            output.run_id,
                            turn_index,
                            comp_idx as i32,
                            comp.summary_item_id,
                            src_id
                        ],
                    )?;
                }
            }

            // Section accounting
            for section in &entry.section_accounting {
                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_section_accounting 
                     (run_id, turn_index, section_id, byte_count, token_count)
                     VALUES (?, ?, ?, ?, ?)",
                    params![
                        output.run_id,
                        turn_index,
                        section.section_id,
                        section.byte_count as i32,
                        section.token_count as i32,
                    ],
                )?;
            }

            // Telemetry
            conn.execute(
                "INSERT OR REPLACE INTO run_turn_telemetry 
                 (run_id, turn_index, prompt_bytes, prompt_tokens, latency_ms, tool_calls)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    output.run_id,
                    turn_index,
                    entry.turn_trace.telemetry.prompt_bytes as i32,
                    entry.turn_trace.telemetry.prompt_tokens as i32,
                    entry.turn_trace.telemetry.latency_ms as i32,
                    entry.turn_trace.telemetry.tool_calls as i32,
                ],
            )?;

            // Turn hashes
            conn.execute(
                "INSERT OR REPLACE INTO run_turn_hashes 
                 (run_id, turn_index, turn_hash)
                 VALUES (?, ?, ?)",
                params![output.run_id, turn_index, entry.turn_trace.hashes.turn_hash],
            )?;

            // Readiness
            conn.execute(
                "INSERT OR REPLACE INTO run_turn_readiness 
                 (run_id, turn_index, readiness_state, readiness_reason)
                 VALUES (?, ?, ?, ?)",
                params![
                    output.run_id,
                    turn_index,
                    format!("{:?}", entry.turn_trace.readiness_state),
                    entry.turn_trace.readiness_reason,
                ],
            )?;

            // Evidence delta
            for (ev_idx, ev_id) in entry.turn_trace.evidence_delta.iter().enumerate() {
                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_evidence_delta 
                     (run_id, turn_index, evidence_index, evidence_id)
                     VALUES (?, ?, ?, ?)",
                    params![output.run_id, turn_index, ev_idx as i32, ev_id],
                )?;
            }

            // Tool traces from TurnLedger
            for (trace_idx, trace) in entry.tool_traces.iter().enumerate() {
                let input_json = serde_json::to_string(&trace.input_payload)?;
                let output_json = serde_json::to_string(&trace.output_payload)?;

                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_tool_traces 
                     (run_id, turn_index, trace_index, tool_name, latency_ms, outcome, input_payload_json, output_payload_json)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        output.run_id,
                        turn_index,
                        trace_idx as i32,
                        trace.tool_name,
                        trace.latency_ms,
                        trace.outcome,
                        input_json,
                        output_json,
                    ],
                )?;
            }
        }

        // Save blobs from ObservabilityBundle
        for turn in &bundle.turns {
            for payload in [
                &turn.request_blob,
                &turn.raw_response_blob,
                &turn.validated_response_blob,
                &turn.prompt_blob,
                &turn.context_blob,
            ] {
                let payload_type =
                    if std::ptr::eq(payload as *const _, &turn.request_blob as *const _) {
                        "request"
                    } else if std::ptr::eq(payload as *const _, &turn.raw_response_blob as *const _)
                    {
                        "raw_response"
                    } else if std::ptr::eq(
                        payload as *const _,
                        &turn.validated_response_blob as *const _,
                    ) {
                        "validated_response"
                    } else if std::ptr::eq(payload as *const _, &turn.prompt_blob as *const _) {
                        "prompt"
                    } else {
                        "context"
                    };

                conn.execute(
                    "INSERT OR REPLACE INTO run_turn_payloads 
                     (run_id, turn_index, payload_type, blob_id, media_type, byte_count, inline_content)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    params![
                        output.run_id,
                        turn.turn_index,
                        payload_type,
                        payload.blob_id,
                        payload.media_type,
                        payload.byte_count as i64,
                        payload.inline_content,
                    ],
                )?;
            }
        }

        // Structured logs
        for (log_idx, log_entry) in bundle.structured_logs.iter().enumerate() {
            let log_level = log_entry
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            let log_component = log_entry
                .get("component")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let log_message = log_entry
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let log_timestamp = log_entry
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let turn_index = log_entry
                .get("turn_index")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let tool_name = log_entry.get("tool_name").and_then(|v| v.as_str());

            conn.execute(
                "INSERT INTO run_structured_logs (run_id, log_index, log_level, log_component, log_message, log_timestamp, turn_index, tool_name) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![output.run_id, log_idx as i32, log_level, log_component, log_message, log_timestamp, turn_index, tool_name],
            )?;
        }

        Ok(())
    }

    pub fn get_run_output(&self, run_id: &str) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT rt.turn_index, rt.rendered_prompt, rt.rendered_context, rt.provider_request_id, rt.telemetry_json,
                    rtt.trace_index, rtt.tool_name, rtt.latency_ms, rtt.outcome, rtt.input_payload_json, rtt.output_payload_json
             FROM run_turns rt
             LEFT JOIN run_turn_tool_traces rtt ON rt.run_id = rtt.run_id AND rt.turn_index = rtt.turn_index
             WHERE rt.run_id = ?
             ORDER BY rt.turn_index, rtt.trace_index"
        )?;

        let mut turns_map: std::collections::HashMap<u32, serde_json::Value> =
            std::collections::HashMap::new();
        let mut rows = stmt.query([run_id])?;

        while let Some(row) = rows.next()? {
            let turn_index: u32 = row.get(0)?;
            let rendered_prompt: String = row.get(1)?;
            let rendered_context: String = row.get(2)?;
            let provider_request_id: Option<String> = row.get(3)?;
            let telemetry_json: String = row.get(4)?;

            let trace_index: Option<i32> = row.get(5)?;
            let tool_name: Option<String> = row.get(6)?;
            let latency_ms: Option<u32> = row.get(7)?;
            let outcome: Option<String> = row.get(8)?;
            let input_payload_json: Option<String> = row.get(9)?;
            let output_payload_json: Option<String> = row.get(10)?;

            let telemetry: graphbench_core::artifacts::TelemetryCounts =
                serde_json::from_str(&telemetry_json).unwrap_or_else(|_| {
                    graphbench_core::artifacts::TelemetryCounts {
                        prompt_bytes: 0,
                        prompt_tokens: 0,
                        latency_ms: 0,
                        tool_calls: 0,
                    }
                });

            let tool_traces: Vec<serde_json::Value> = if let (
                Some(idx),
                Some(name),
                Some(lat),
                Some(out),
                Some(inp),
                Some(oup),
            ) = (
                trace_index,
                tool_name,
                latency_ms,
                outcome,
                input_payload_json,
                output_payload_json,
            ) {
                vec![serde_json::json!({
                    "tool_name": name,
                    "latency_ms": lat,
                    "outcome": out,
                    "input_payload": serde_json::from_str(&inp).unwrap_or(serde_json::Value::Null),
                    "output_payload": serde_json::from_str(&oup).unwrap_or(serde_json::Value::Null),
                })]
            } else {
                vec![]
            };

            let entry = serde_json::json!({
                "turn_index": turn_index,
                "rendered_prompt": rendered_prompt,
                "rendered_context": rendered_context,
                "provider_request_id": provider_request_id,
                "telemetry": telemetry,
                "tool_traces": tool_traces,
                "readiness_state": "",
                "evidence_delta": Vec::<serde_json::Value>::new(),
            });

            turns_map.insert(turn_index, entry);
        }

        drop(rows);
        drop(stmt);

        let mut readiness_stmt = conn.prepare(
            "SELECT turn_index, readiness_state, readiness_reason FROM run_turn_readiness WHERE run_id = ?"
        )?;
        let mut readiness_rows = readiness_stmt.query([run_id])?;
        while let Some(row) = readiness_rows.next()? {
            let turn_index: u32 = row.get(0)?;
            let readiness_state: String = row.get(1)?;
            let readiness_reason: String = row.get(2)?;
            if let Some(entry) = turns_map.get_mut(&turn_index) {
                entry["readiness_state"] = serde_json::json!(readiness_state);
                entry["readiness_reason"] = serde_json::json!(readiness_reason);
            }
        }

        drop(readiness_rows);
        drop(readiness_stmt);

        let mut evidence_stmt = conn.prepare(
            "SELECT turn_index, evidence_index, evidence_id FROM run_turn_evidence_delta WHERE run_id = ? ORDER BY turn_index, evidence_index"
        )?;
        let mut evidence_rows = evidence_stmt.query([run_id])?;
        let mut current_turn: Option<u32> = None;
        let mut current_evidence: Vec<serde_json::Value> = Vec::new();
        while let Some(row) = evidence_rows.next()? {
            let turn_index: u32 = row.get(0)?;
            let evidence_id: String = row.get(2)?;
            if current_turn != Some(turn_index) {
                if let Some(tidx) = current_turn {
                    if let Some(entry) = turns_map.get_mut(&tidx) {
                        entry["evidence_delta"] = serde_json::json!(current_evidence.clone());
                    }
                }
                current_turn = Some(turn_index);
                current_evidence = Vec::new();
            }
            current_evidence.push(serde_json::json!(evidence_id));
        }
        if let Some(tidx) = current_turn {
            if let Some(entry) = turns_map.get_mut(&tidx) {
                entry["evidence_delta"] = serde_json::json!(current_evidence);
            }
        }

        let entries: Vec<serde_json::Value> = turns_map
            .into_iter()
            .map(|(idx, mut entry)| {
                entry["turn_trace"] = serde_json::json!({
                    "run_id": run_id,
                    "turn_index": idx,
                    "task_id": "",
                    "fixture_id": "",
                    "strategy_id": "",
                });
                entry.as_object_mut().map(|m| m.remove("turn_index"));
                entry
            })
            .collect();

        let turn_ledger = serde_json::json!({
            "run_id": run_id,
            "task_id": "",
            "fixture_id": "",
            "entries": entries,
        });

        Ok(Some(turn_ledger))
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
