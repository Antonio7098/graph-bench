use axum::{
    routing::{get, post},
    extract::{Path, State, Query},
    response::{IntoResponse, Json, Response},
    Router,
};
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
        // Strategies
        .route("/api/strategies", get(list_strategies))
        .route("/api/strategies", post(create_strategy))
        .route("/api/strategies/all", get(list_strategies_with_versions))
        .route("/api/strategies/:id", get(get_strategy))
        .route("/api/strategies/:id/versions", get(list_strategy_versions))
        // Tasks
        .route("/api/tasks", get(list_tasks))
        .route("/api/tasks", post(create_task))
        .route("/api/tasks/all", get(list_tasks_with_versions))
        .route("/api/tasks/:id", get(get_task))
        .route("/api/tasks/:id/versions", get(list_task_versions))
        // Evidence
        .route("/api/evidence", get(list_evidence))
        .route("/api/evidence", post(create_evidence))
        .route("/api/evidence/:id", get(get_evidence))
        // Fixtures
        .route("/api/fixtures", get(list_fixtures))
        .route("/api/fixtures", post(create_fixture))
        .route("/api/fixtures/all", get(list_fixtures_with_versions))
        .route("/api/fixtures/:id", get(get_fixture))
        .route("/api/fixtures/:id/versions", get(list_fixture_versions))
        // Prompts
        .route("/api/prompts", get(list_prompts))
        .route("/api/prompts", post(create_prompt))
        .route("/api/prompts/all", get(list_prompts_with_versions))
        .route("/api/prompts/:id", get(get_prompt))
        .route("/api/prompts/:id/versions", get(list_prompt_versions))
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
    // First try to get from DB runs table
    let runs = match state.db.list_runs(None) {
        Ok(runs) => runs,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    
    if let Some(run_summary) = runs.into_iter().find(|r| r.run_id == id) {
        // Build minimal response from runs table
        Json(serde_json::json!({
            "manifest": {
                "run_id": run_summary.run_id,
                "fixture_id": run_summary.fixture_id,
                "task_id": run_summary.task_id,
                "strategy_id": run_summary.strategy_id,
                "provider": run_summary.provider,
                "model_slug": run_summary.model_slug,
                "started_at": run_summary.started_at,
                "completed_at": run_summary.completed_at,
                "outcome": run_summary.status,
            },
            "turns": [],
            "evidence_matches": [],
            "score_report": run_summary.visibility_score.map(|v| serde_json::json!({
                "evidence_visibility_score": v,
                "evidence_acquisition_score": run_summary.acquisition_score.unwrap_or(0.0),
                "evidence_efficiency_score": run_summary.efficiency_score.unwrap_or(0.0),
                "explanation_quality_score": run_summary.explanation_score.unwrap_or(0.0),
            })),
        })).into_response()
    } else {
        // Fall back to trying to load from trace files
        match state.db.get_run(&id) {
            Ok(Some(run)) => Json(run).into_response(),
            Ok(None) => (axum::http::StatusCode::NOT_FOUND, "Run not found").into_response(),
            Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
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
            Ok((run_id, _, run_data)) => {
                if let Err(e) = db.save_run_output(&run_data) {
                    tracing::warn!("Failed to save run output to DB: {}", e);
                }
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
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let version = query.get("version").and_then(|v| v.parse().ok());
    match state.db.get_strategy(&id, version) {
        Ok(Some(config)) => Json(config).into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "Strategy not found").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let version = query.get("version").and_then(|v| v.parse().ok());
    match state.db.get_task(&id, version) {
        Ok(Some(spec)) => Json(spec).into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "Task not found").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn get_evidence(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let version = query.get("version").and_then(|v| v.parse().ok());
    match state.db.get_evidence(&id, version) {
        Ok(Some(spec)) => Json(spec).into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "Evidence not found").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn get_fixture(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let version = query.get("version").and_then(|v| v.parse().ok());
    match state.db.get_fixture(&id, version) {
        Ok(Some(config)) => Json(config).into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "Fixture not found").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn get_prompt(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let version = query.get("version").and_then(|v| v.parse().ok());
    match state.db.get_prompt(&id, version) {
        Ok(Some(template)) => Json(template).into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "Prompt not found").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_strategies(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_strategies() {
        Ok(strategies) => Json(strategies).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_strategies_with_versions(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_strategies_with_versions() {
        Ok(strategies) => Json(strategies).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_tasks(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_tasks() {
        Ok(tasks) => Json(tasks).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_tasks_with_versions(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_tasks_with_versions() {
        Ok(tasks) => Json(tasks).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_fixtures(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_fixtures() {
        Ok(fixtures) => Json(fixtures).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_fixtures_with_versions(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_fixtures_with_versions() {
        Ok(fixtures) => Json(fixtures).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_prompts(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_prompts() {
        Ok(prompts) => Json(prompts).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_prompts_with_versions(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_prompts_with_versions() {
        Ok(prompts) => Json(prompts).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_strategy_versions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.db.list_strategy_versions(&id) {
        Ok(versions) => Json(versions).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_task_versions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.db.list_task_versions(&id) {
        Ok(versions) => Json(versions).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_fixture_versions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.db.list_fixture_versions(&id) {
        Ok(versions) => Json(versions).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_prompt_versions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.db.list_prompt_versions(&id) {
        Ok(versions) => Json(versions).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ============ CREATE ENDPOINTS ============

#[derive(serde::Deserialize)]
pub struct CreateStrategyRequest {
    pub name: String,
    pub config: serde_json::Value,
    pub description: Option<String>,
}

pub async fn create_strategy(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateStrategyRequest>,
) -> Response {
    match state.db.insert_strategy(&req.name, &req.config, req.description.as_deref()) {
        Ok(version) => Json(json!({ "name": req.name, "version": version })).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct CreateTaskRequest {
    pub task_id: String,
    pub spec: serde_json::Value,
}

pub async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTaskRequest>,
) -> Response {
    match state.db.insert_task(&req.task_id, &req.spec) {
        Ok(version) => Json(json!({ "task_id": req.task_id, "version": version })).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct CreateEvidenceRequest {
    pub task_id: String,
    pub evidence_id: String,
    pub spec: serde_json::Value,
}

pub async fn create_evidence(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateEvidenceRequest>,
) -> Response {
    match state.db.insert_evidence(&req.task_id, &req.evidence_id, &req.spec) {
        Ok(version) => Json(json!({ "evidence_id": req.evidence_id, "version": version })).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_evidence(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_all_evidence() {
        Ok(evidence) => Json(evidence).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct CreateFixtureRequest {
    pub name: String,
    pub config: serde_json::Value,
    pub graph_snapshot: Option<serde_json::Value>,
}

pub async fn create_fixture(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateFixtureRequest>,
) -> Response {
    match state.db.insert_fixture(&req.name, &req.config, req.graph_snapshot.as_ref()) {
        Ok(version) => Json(json!({ "name": req.name, "version": version })).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct CreatePromptRequest {
    pub name: String,
    pub template: serde_json::Value,
    pub description: Option<String>,
}

pub async fn create_prompt(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreatePromptRequest>,
) -> Response {
    match state.db.insert_prompt(&req.name, &req.template, req.description.as_deref()) {
        Ok(version) => Json(json!({ "name": req.name, "version": version })).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

