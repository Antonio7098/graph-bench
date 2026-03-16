use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Query, State},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WsParams {
    pub run_id: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(params): Query<WsParams>,
) -> impl IntoResponse {
    let rx = state.event_stream.subscribe();
    let replay = state.event_stream.replay(params.run_id.as_deref());
    ws.on_upgrade(move |socket| handle_socket(socket, rx, replay, params.run_id))
}

async fn handle_socket(
    socket: WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<crate::event_stream::StreamEvent>,
    replay: Vec<crate::event_stream::StreamEvent>,
    run_id_filter: Option<String>,
) {
    let (sender, mut receiver) = socket.split();
    
    let sender = Arc::new(tokio::sync::Mutex::new(sender));

    for event in replay {
        let Ok(payload) = serde_json::to_string(&event) else {
            continue;
        };
        let mut s = sender.lock().await;
        if s.send(Message::Text(payload.into())).await.is_err() {
            return;
        }
    }
    
    let sender_clone = sender.clone();
    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(data) => {
                    if let Some(run_id) = run_id_filter.as_deref() {
                        if data.run_id.as_deref() != Some(run_id) {
                            continue;
                        }
                    }
                    let Ok(payload) = serde_json::to_string(&data) else {
                        continue;
                    };
                    let mut s = sender_clone.lock().await;
                    if s.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!("WebSocket lagged {} messages", n);
                }
                Err(RecvError::Closed) => {
                    break;
                }
            }
        }
    });
    
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Ping(data)) => {
                    let mut s = sender.lock().await;
                    if s.send(Message::Pong(data)).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
            }
        }
    });
    
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
}
