use axum::{
    routing::{get, post},
    extract::{Path, State, Query},
    response::{IntoResponse, Json, Response},
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use serde_json::json;

use crate::db::RunFilter;
use crate::AppState;

pub fn run_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/runs", get(list_runs))
        .route("/api/runs/:id", get(get_run))
        .route("/api/runs/:id/events", get(get_run_events))
        .route("/api/runs/run", post(start_run))
}

async fn list_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let filter = RunFilter {
        fixture_id: query.get("fixture_id").cloned(),
        task_id: query.get("task_id").cloned(),
        strategy_id: query.get("strategy_id").cloned(),
        outcome: query.get("outcome").cloned(),
    };
    
    match state.db.list_runs(Some(filter)) {
        Ok(runs) => Json(runs).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.db.get_run(&id) {
        Ok(Some(run)) => Json(run).into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "Run not found").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_run_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let from_seq = query.get("from_seq").and_then(|s| s.parse::<u64>().ok());
    
    // First try to get from DB
    match state.db.get_events_for_run(&id, from_seq) {
        Ok(events) if !events.is_empty() => {
            return Json(events).into_response();
        }
        _ => {}
    }
    
    // Fall back to in-memory replay
    let events = state.event_stream.replay(Some(&id));
    Json(events).into_response()
}

#[derive(serde::Deserialize)]
pub struct RunRequest {
    pub task_spec_path: Option<String>,
    pub fixture_path: Option<String>,
    pub model_id: Option<String>,
    pub api_key: Option<String>,
    pub strategy: Option<String>,
    pub turn_budget: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub token_budget: Option<u32>,
    pub prompt_headroom: Option<u32>,
    pub seed_overview: Option<u32>,
    pub initial_select: Option<String>,
    pub representation_level: Option<String>,
}

#[derive(serde::Serialize)]
pub struct RunResponse {
    pub success: bool,
    pub run_id: Option<String>,
    pub output: Option<String>,
}

async fn start_run(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunRequest>,
) -> Response {
    let run_id = format!(
        "benchmark-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should move forward")
            .as_secs()
    );
    let config = crate::harness::BenchmarkConfig {
        run_id: run_id.clone(),
        task_spec_path: req.task_spec_path.unwrap_or_else(|| "tasks/prepare-to-edit/task-01.task.json".to_string()),
        fixture_path: req.fixture_path.unwrap_or_else(|| "fixtures/graphbench-internal/fixture.json".to_string()),
        model_id: req.model_id,
        api_key: req.api_key,
        strategy: req.strategy.unwrap_or_else(|| "graph_then_targeted_lexical_read".to_string()),
        turn_budget: req.turn_budget.unwrap_or(48),
        timeout_ms: req.timeout_ms.unwrap_or(300_000),
        token_budget: req.token_budget.unwrap_or(2_000_000),
        prompt_headroom: req.prompt_headroom.unwrap_or(24_576),
        seed_overview: req.seed_overview.unwrap_or(2),
        initial_select: req.initial_select.unwrap_or_else(|| "crates/graphbench-core/src/artifacts.rs".to_string()),
        representation_level: req.representation_level.unwrap_or_else(|| "L1".to_string()),
    };
    
    let traces_dir = state.traces_dir.clone();
    let event_stream = state.event_stream.clone();
    let db = state.db.clone();
    
    // Insert in-progress run status to DB
    if let Err(e) = db.upsert_run_status(&run_id, "running", None) {
        tracing::warn!("Failed to insert run status to DB: {}", e);
    }
    
    tracing::info!("Starting run: task={}, model={:?}", config.task_spec_path, config.model_id);
    
    let run_id_clone = run_id.clone();

    event_stream.publish(crate::event_stream::StreamEvent {
        seq: 0,
        captured_at: crate::event_stream::now_rfc3339(),
        stream: "live".to_owned(),
        run_id: Some(run_id.clone()),
        component: "api".to_owned(),
        event_type: "run.accepted".to_owned(),
        level: "info".to_owned(),
        message: format!("Accepted benchmark run {}", run_id),
        turn_index: None,
        tool_name: None,
        provider_request_id: None,
        metrics: None,
        tags: vec!["api".to_owned(), "lifecycle".to_owned()],
        details: json!({
            "task_spec_path": config.task_spec_path,
            "fixture_path": config.fixture_path,
            "strategy": config.strategy,
            "model_id": config.model_id,
            "turn_budget": config.turn_budget,
            "timeout_ms": config.timeout_ms,
            "token_budget": config.token_budget,
            "prompt_headroom": config.prompt_headroom,
        }),
    });
    
    tokio::spawn(async move {
        match crate::harness::run_benchmark(config, event_stream.clone()).await {
            Ok((run_id, _)) => {
                let _ = db.import_traces(&traces_dir);
                let _ = db.upsert_run_status(&run_id.as_str(), "completed", None);
                tracing::info!("Run completed: {}", run_id);
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                tracing::error!("[{}] Run failed: {}", run_id, error_msg);
                
                // Update run status to failed
                let _ = db.upsert_run_status(run_id.as_str(), "failed", Some(&error_msg));
                
                // Send error event to WebSocket
                event_stream.publish(crate::event_stream::StreamEvent {
                    seq: 0,
                    captured_at: crate::event_stream::now_rfc3339(),
                    stream: "live".to_owned(),
                    run_id: Some(run_id.clone()),
                    component: "api".to_owned(),
                    event_type: "run.failed".to_owned(),
                    level: "error".to_owned(),
                    message: error_msg.clone(),
                    turn_index: None,
                    tool_name: None,
                    provider_request_id: None,
                    metrics: None,
                    tags: vec!["api".to_owned(), "error".to_owned()],
                    details: serde_json::json!({ "error": error_msg }),
                });
            }
        }
    });
    
    Json(RunResponse {
        success: true,
        run_id: Some(run_id_clone.clone()),
        output: Some(format!("Run {} started", run_id_clone)),
    }).into_response()
}

pub async fn get_strategy(
    Path(id): Path<String>,
) -> Response {
    let strategies_dir = PathBuf::from("strategies");
    
    if let Ok(entries) = std::fs::read_dir(&strategies_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with(&id) && filename.ends_with(".json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                            return Json(data).into_response();
                        }
                    }
                }
            }
        }
    }
    
    (axum::http::StatusCode::NOT_FOUND, "Strategy not found").into_response()
}

pub async fn get_task(
    Path(id): Path<String>,
) -> Response {
    let tasks_dir = PathBuf::from("tasks");
    
    if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            
            if let Ok(subentries) = std::fs::read_dir(&path) {
                for subentry in subentries.flatten() {
                    let subpath = subentry.path();
                    if let Some(filename) = subpath.file_name().and_then(|n| n.to_str()) {
                        if filename.ends_with(".task.json") {
                            if let Ok(content) = std::fs::read_to_string(&subpath) {
                                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                                    if data.get("task_id").and_then(|v| v.as_str()) == Some(&id) {
                                        return Json(data).into_response();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    (axum::http::StatusCode::NOT_FOUND, "Task not found").into_response()
}

pub async fn list_strategies() -> Response {
    let strategies_dir = PathBuf::from("strategies");
    let mut strategies = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&strategies_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(filename) = path.file_stem().and_then(|n| n.to_str()) {
                    strategies.push(filename.to_string());
                }
            }
        }
    }
    
    strategies.sort();
    Json(strategies).into_response()
}

pub async fn list_tasks() -> Response {
    let tasks_dir = PathBuf::from("tasks");
    let mut task_paths = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            
            if let Ok(subentries) = std::fs::read_dir(&path) {
                for subentry in subentries.flatten() {
                    let subpath = subentry.path();
                    if subpath.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Some(filename) = subpath.file_name().and_then(|n| n.to_str()) {
                            if filename.ends_with(".task.json") {
                                if let Ok(rel_path) = subpath.strip_prefix(&tasks_dir) {
                                    task_paths.push(rel_path.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    task_paths.sort();
    Json(task_paths).into_response()
}

pub async fn list_fixtures() -> Response {
    let fixtures_dir = PathBuf::from("fixtures");
    let mut fixture_paths = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&fixtures_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            
            if let Ok(subentries) = std::fs::read_dir(&path) {
                for subentry in subentries.flatten() {
                    let subpath = subentry.path();
                    if subpath.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Ok(rel_path) = subpath.strip_prefix(&fixtures_dir) {
                            fixture_paths.push(rel_path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }
    
    fixture_paths.sort();
    Json(fixture_paths).into_response()
}

