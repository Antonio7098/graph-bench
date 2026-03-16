#![allow(unsafe_code)]

mod db;
mod api;
mod event_stream;
mod harness;
mod websocket;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::db::Database;
use crate::api::run_routes;
use crate::event_stream::EventStream;
use crate::websocket::ws_handler;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub event_stream: Arc<EventStream>,
    pub traces_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,graphbench_api=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse::<u16>()?;

    let traces_dir = std::env::var("TRACES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("traces"));

    let data_dir = std::env::var("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"));

    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&traces_dir)?;

    let db_path = data_dir.join("graphbench.db");
    let db = Arc::new(Database::new(&db_path)?);

    let event_stream = EventStream::with_db(db.clone());

    let state = AppState {
        db,
        event_stream,
        traces_dir,
    };

    let state = Arc::new(state);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(run_routes())
        .route("/ws", axum::routing::get(ws_handler))
        .route("/api/strategies", axum::routing::get(api::list_strategies))
        .route("/api/strategies/:id", axum::routing::get(api::get_strategy))
        .route("/api/tasks", axum::routing::get(api::list_tasks))
        .route("/api/tasks/:id", axum::routing::get(api::get_task))
        .route("/api/fixtures", axum::routing::get(api::list_fixtures))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("API server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
