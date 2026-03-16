use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::db::Database;

const MAX_EVENT_HISTORY: usize = 5000;

#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    pub seq: u64,
    pub captured_at: String,
    pub stream: String,
    pub run_id: Option<String>,
    pub component: String,
    pub event_type: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug)]
pub struct EventStream {
    tx: broadcast::Sender<StreamEvent>,
    history: Mutex<VecDeque<StreamEvent>>,
    seq: AtomicU64,
    db: Option<Arc<Database>>,
}

impl EventStream {
    pub fn new(capacity: usize) -> Arc<Self> {
        let (tx, _) = broadcast::channel(capacity);
        Arc::new(Self {
            tx,
            history: Mutex::new(VecDeque::with_capacity(MAX_EVENT_HISTORY)),
            seq: AtomicU64::new(1),
            db: None,
        })
    }

    pub fn with_db(db: Arc<Database>) -> Arc<Self> {
        let (tx, _) = broadcast::channel(2000);
        Arc::new(Self {
            tx,
            history: Mutex::new(VecDeque::with_capacity(MAX_EVENT_HISTORY)),
            seq: AtomicU64::new(1),
            db: Some(db),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.tx.subscribe()
    }

    pub fn publish(&self, mut event: StreamEvent) {
        event.seq = self.seq.fetch_add(1, Ordering::Relaxed);

        // Store in history (in-memory)
        {
            let mut history = self.history.lock().expect("event history lock");
            if history.len() >= MAX_EVENT_HISTORY {
                history.pop_front();
            }
            history.push_back(event.clone());
        }

        // Also persist to database if available
        if let Some(db) = &self.db {
            if let Some(run_id) = &event.run_id {
                let db_event = crate::db::RunEvent {
                    run_id: run_id.clone(),
                    seq: event.seq as i64,
                    captured_at: event.captured_at.clone(),
                    stream: event.stream.clone(),
                    component: event.component.clone(),
                    event_type: event.event_type.clone(),
                    level: event.level.clone(),
                    message: event.message.clone(),
                    turn_index: event.turn_index.map(|v| v as i32),
                    tool_name: event.tool_name.clone(),
                    provider_request_id: event.provider_request_id.clone(),
                    metrics: event.metrics.clone(),
                    tags: event.tags.clone(),
                    details: event.details.clone(),
                };
                if let Err(e) = db.insert_event(&db_event) {
                    tracing::error!("Failed to persist event to DB: {}", e);
                }
            }
        }

        let _ = self.tx.send(event);
    }

    pub fn replay(&self, run_id: Option<&str>) -> Vec<StreamEvent> {
        let history = self.history.lock().expect("event history lock");
        history
            .iter()
            .filter(|event| match run_id {
                Some(run_id) => event.run_id.as_deref() == Some(run_id),
                None => true,
            })
            .cloned()
            .collect()
    }
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}
