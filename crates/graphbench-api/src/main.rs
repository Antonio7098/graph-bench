#![allow(unsafe_code)]

mod db;
mod api;
mod harness;
mod websocket;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use axum::Router;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::db::Database;
use crate::api::run_routes;
use crate::websocket::ws_handler;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub event_tx: broadcast::Sender<String>,
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

    let (event_tx, _) = broadcast::channel::<String>(1000);

    let state = AppState {
        db,
        event_tx,
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
        .route("/api/strategies/:id", axum::routing::get(api::get_strategy))
        .route("/api/tasks/:id", axum::routing::get(api::get_task))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("API server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
