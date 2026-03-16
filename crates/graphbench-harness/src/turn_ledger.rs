use crate::runtime::RuntimeLoopState;
use crate::tools::ToolCallTrace;
use graphbench_core::artifacts::TurnTrace;
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerSectionAccounting {
    pub section_id: String,
    pub byte_count: u32,
    pub token_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionRecord {
    pub summary_item_id: String,
    pub source_item_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeStreamKind {
    Objective {
        text: String,
    },
    Assistant {
        text: String,
    },
    ToolCall {
        tool_name: String,
        payload: serde_json::Value,
    },
    ToolResult {
        tool_name: String,
        payload: serde_json::Value,
    },
    EditEventSummary {
        text: String,
    },
    Summary {
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStreamItem {
    pub item_id: String,
    pub payload: RuntimeStreamKind,
}

impl RuntimeStreamItem {
    pub fn objective(text: String) -> Self {
        Self {
            item_id: "stream-objective-0".to_owned(),
            payload: RuntimeStreamKind::Objective { text },
        }
    }

    pub fn assistant(text: String) -> Self {
        Self {
            item_id: format!("stream-assistant-{}", sha256_string(&text)),
            payload: RuntimeStreamKind::Assistant { text },
        }
    }

    pub fn tool_call(tool_name: String, payload: serde_json::Value) -> Self {
        Self {
            item_id: format!("stream-tool-call-{}", tool_name),
            payload: RuntimeStreamKind::ToolCall { tool_name, payload },
        }
    }

    pub fn tool_result(tool_name: String, payload: serde_json::Value) -> Self {
        Self {
            item_id: format!("stream-tool-result-{}", tool_name),
            payload: RuntimeStreamKind::ToolResult { tool_name, payload },
        }
    }

    pub fn summary(text: String) -> Self {
        Self {
            item_id: format!("stream-summary-{}", sha256_string(&text)),
            payload: RuntimeStreamKind::Summary { text },
        }
    }

    pub fn edit_event_summary(text: String) -> Self {
        Self {
            item_id: format!("stream-edit-summary-{}", sha256_string(&text)),
            payload: RuntimeStreamKind::EditEventSummary { text },
        }
    }

    pub fn render(&self) -> String {
        match &self.payload {
            RuntimeStreamKind::Objective { text }
            | RuntimeStreamKind::Assistant { text }
            | RuntimeStreamKind::EditEventSummary { text }
            | RuntimeStreamKind::Summary { text } => text.clone(),
            RuntimeStreamKind::ToolCall { tool_name, payload } => {
                format!("tool_call {} {}", tool_name, summarize_payload(payload))
            }
            RuntimeStreamKind::ToolResult { tool_name, payload } => {
                format!("tool_result {} {}", tool_name, summarize_payload(payload))
            }
        }
    }

    pub fn render_for_summary(&self) -> String {
        match &self.payload {
            RuntimeStreamKind::ToolResult { tool_name, payload } => {
                format!(
                    "tool_result_summary {} {}",
                    tool_name,
                    summarize_payload(payload)
                )
            }
            RuntimeStreamKind::ToolCall { tool_name, payload } => {
                format!(
                    "tool_call_summary {} {}",
                    tool_name,
                    summarize_payload(payload)
                )
            }
            _ => self.render(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnLedgerEntry {
    pub turn_trace: TurnTrace,
    pub state_before: RuntimeLoopState,
    pub state_after: RuntimeLoopState,
    pub graph_session_before: String,
    pub graph_session_after: String,
    pub ordered_context_object_ids: Vec<String>,
    pub compactions: Vec<CompactionRecord>,
    pub section_accounting: Vec<LedgerSectionAccounting>,
    pub rendered_prompt: String,
    pub rendered_context: String,
    pub tool_traces: Vec<ToolCallTrace>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnLedger {
    pub run_id: String,
    pub task_id: String,
    pub fixture_id: String,
    pub entries: Vec<TurnLedgerEntry>,
}

impl TurnLedger {
    pub fn new(run_id: String, task_id: String, fixture_id: String) -> Self {
        Self {
            run_id,
            task_id,
            fixture_id,
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, entry: TurnLedgerEntry) -> Result<(), AppError> {
        let replay_hash = sha256_string(&entry.rendered_prompt);
        if replay_hash != entry.replay_hash {
            return Err(AppError::new(
                ErrorCode::ContextReconstructionFailed,
                "turn ledger replay hash does not match rendered prompt",
                ErrorContext {
                    component: "turn_ledger",
                    operation: "push",
                },
            ));
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), AppError> {
        let payload = serde_json::to_string_pretty(self).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                "failed to serialize turn ledger",
                ErrorContext {
                    component: "turn_ledger",
                    operation: "save",
                },
                source,
            )
        })?;
        fs::write(path.as_ref(), payload).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                format!(
                    "failed to persist turn ledger to {}",
                    path.as_ref().display()
                ),
                ErrorContext {
                    component: "turn_ledger",
                    operation: "save",
                },
                source,
            )
        })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let payload = fs::read_to_string(path.as_ref()).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                format!(
                    "failed to read turn ledger from {}",
                    path.as_ref().display()
                ),
                ErrorContext {
                    component: "turn_ledger",
                    operation: "load",
                },
                source,
            )
        })?;
        serde_json::from_str(&payload).map_err(|source| {
            AppError::with_source(
                ErrorCode::SchemaValidationFailed,
                "failed to parse turn ledger",
                ErrorContext {
                    component: "turn_ledger",
                    operation: "load",
                },
                source,
            )
        })
    }

    pub fn replay_validate(&self) -> Result<(), AppError> {
        for entry in &self.entries {
            entry.turn_trace.validate_for_replay()?;
            if sha256_string(&entry.rendered_prompt) != entry.replay_hash {
                return Err(AppError::new(
                    ErrorCode::ContextReconstructionFailed,
                    "turn replay hash mismatch",
                    ErrorContext {
                        component: "turn_ledger",
                        operation: "replay_validate",
                    },
                ));
            }
            if entry.turn_trace.request.prompt_hash != sha256_string(&entry.rendered_prompt) {
                return Err(AppError::new(
                    ErrorCode::ContextReconstructionFailed,
                    "stored prompt hash does not match replayed prompt",
                    ErrorContext {
                        component: "turn_ledger",
                        operation: "replay_validate",
                    },
                ));
            }
            if entry.turn_trace.request.context_hash != sha256_string(&entry.rendered_context) {
                return Err(AppError::new(
                    ErrorCode::ContextReconstructionFailed,
                    "stored context hash does not match replayed context",
                    ErrorContext {
                        component: "turn_ledger",
                        operation: "replay_validate",
                    },
                ));
            }
        }
        Ok(())
    }
}

fn sha256_string(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!("sha256:{digest:x}")
}

fn summarize_payload(payload: &serde_json::Value) -> String {
    match payload {
        serde_json::Value::Object(map) => {
            let fields = map.keys().cloned().collect::<Vec<_>>().join(",");
            let status = map
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let path = map
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let line_count = map
                .get("line_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let match_count = map
                .get("matches")
                .and_then(|value| value.as_array())
                .map_or(0, std::vec::Vec::len);
            format!(
                "{{status:{status},path:{path},line_count:{line_count},match_count:{match_count},fields:[{fields}]}}"
            )
        }
        _ => payload.to_string(),
    }
}
