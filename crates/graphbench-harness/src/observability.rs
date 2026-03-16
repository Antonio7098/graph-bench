use crate::runtime::{
    HarnessEvent, HarnessInput, HarnessModelResponse, HarnessOutput, ModelInvocation,
};
use crate::tools::ToolCallTrace;
use graphbench_core::artifacts::{
    RunManifest, RunSchemaVersionSet, TURN_TRACE_SCHEMA_VERSION, TelemetryCounts,
};
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadBlobRef {
    pub blob_id: String,
    pub media_type: String,
    pub path: String,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityEventRecord {
    pub event_id: String,
    pub captured_at: String,
    pub run_id: String,
    pub task_id: String,
    pub fixture_id: String,
    pub strategy_id: String,
    pub turn_index: Option<u32>,
    pub component: String,
    pub event_type: String,
    pub details: Value,
    pub blob_refs: Vec<PayloadBlobRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnTelemetryCapture {
    pub turn_index: u32,
    pub request_blob: PayloadBlobRef,
    pub raw_response_blob: PayloadBlobRef,
    pub validated_response_blob: PayloadBlobRef,
    pub prompt_blob: PayloadBlobRef,
    pub context_blob: PayloadBlobRef,
    pub provider_request_id: Option<String>,
    pub telemetry: TelemetryCounts,
    pub tool_traces: Vec<ToolCallTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunTelemetryAggregation {
    pub total_turns: u32,
    pub aggregate_prompt_bytes: u64,
    pub aggregate_prompt_tokens: u64,
    pub aggregate_latency_ms: u64,
    pub aggregate_tool_calls: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityBundle {
    pub run_manifest: RunManifest,
    pub events: Vec<ObservabilityEventRecord>,
    pub turns: Vec<TurnTelemetryCapture>,
    pub aggregation: RunTelemetryAggregation,
    pub structured_logs: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedEvent {
    pub captured_at: String,
    pub event: HarnessEvent,
}

impl ObservabilityBundle {
    pub fn validate(&self, root: impl AsRef<Path>) -> Result<(), AppError> {
        self.run_manifest.validate()?;
        if self.turns.is_empty() {
            return Err(AppError::new(
                ErrorCode::SchemaValidationFailed,
                "observability bundle must contain turn telemetry",
                ErrorContext {
                    component: "observability",
                    operation: "validate",
                },
            ));
        }
        if self.events.is_empty() {
            return Err(AppError::new(
                ErrorCode::SchemaValidationFailed,
                "observability bundle must contain event records",
                ErrorContext {
                    component: "observability",
                    operation: "validate",
                },
            ));
        }
        if self.aggregation.total_turns != self.turns.len() as u32 {
            return Err(AppError::new(
                ErrorCode::SchemaValidationFailed,
                "observability aggregation turn count does not match turn telemetry",
                ErrorContext {
                    component: "observability",
                    operation: "validate",
                },
            ));
        }
        for event in &self.events {
            if event.run_id != self.run_manifest.run_id {
                return Err(AppError::new(
                    ErrorCode::SchemaValidationFailed,
                    "observability event run_id mismatch",
                    ErrorContext {
                        component: "observability",
                        operation: "validate",
                    },
                ));
            }
            for blob in &event.blob_refs {
                if !root.as_ref().join(&blob.path).exists() {
                    return Err(AppError::new(
                        ErrorCode::SchemaValidationFailed,
                        format!("missing event blob {}", blob.path),
                        ErrorContext {
                            component: "observability",
                            operation: "validate",
                        },
                    ));
                }
            }
        }
        for turn in &self.turns {
            for blob in [
                &turn.request_blob,
                &turn.raw_response_blob,
                &turn.validated_response_blob,
                &turn.prompt_blob,
                &turn.context_blob,
            ] {
                if !root.as_ref().join(&blob.path).exists() {
                    return Err(AppError::new(
                        ErrorCode::SchemaValidationFailed,
                        format!("missing turn blob {}", blob.path),
                        ErrorContext {
                            component: "observability",
                            operation: "validate",
                        },
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), AppError> {
        let payload = serde_json::to_string_pretty(self).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                "failed to serialize observability bundle",
                ErrorContext {
                    component: "observability",
                    operation: "save",
                },
                source,
            )
        })?;
        fs::write(path.as_ref(), payload).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                format!(
                    "failed to save observability bundle to {}",
                    path.as_ref().display()
                ),
                ErrorContext {
                    component: "observability",
                    operation: "save",
                },
                source,
            )
        })
    }

    pub fn save_structured_logs_jsonl(&self, path: impl AsRef<Path>) -> Result<(), AppError> {
        let mut file = fs::File::create(path.as_ref()).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                format!(
                    "failed to create structured log output at {}",
                    path.as_ref().display()
                ),
                ErrorContext {
                    component: "observability",
                    operation: "save_structured_logs_jsonl",
                },
                source,
            )
        })?;
        for record in &self.structured_logs {
            let line = serde_json::to_string(record).map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to serialize structured log record",
                    ErrorContext {
                        component: "observability",
                        operation: "save_structured_logs_jsonl",
                    },
                    source,
                )
            })?;
            writeln!(file, "{line}").map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    "failed to write structured log record",
                    ErrorContext {
                        component: "observability",
                        operation: "save_structured_logs_jsonl",
                    },
                    source,
                )
            })?;
        }
        Ok(())
    }
}

pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, AppError> {
        fs::create_dir_all(root.as_ref()).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                format!("failed to create blob store {}", root.as_ref().display()),
                ErrorContext {
                    component: "observability",
                    operation: "create_blob_store",
                },
                source,
            )
        })?;
        Ok(Self {
            root: root.as_ref().to_path_buf(),
        })
    }

    pub fn write_text(
        &self,
        namespace: &str,
        media_type: &str,
        content: &str,
    ) -> Result<PayloadBlobRef, AppError> {
        let blob_id = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
        let dir = self.root.join(namespace);
        fs::create_dir_all(&dir).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                format!("failed to create namespace {}", dir.display()),
                ErrorContext {
                    component: "observability",
                    operation: "create_blob_namespace",
                },
                source,
            )
        })?;
        let extension = if media_type.contains("json") {
            "json"
        } else {
            "txt"
        };
        let filename = format!("{}.{}", blob_id.trim_start_matches("sha256:"), extension);
        let path = dir.join(filename);
        if !path.exists() {
            fs::write(&path, content).map_err(|source| {
                AppError::with_source(
                    ErrorCode::PersistenceWriteFailed,
                    format!("failed to write blob {}", path.display()),
                    ErrorContext {
                        component: "observability",
                        operation: "write_blob",
                    },
                    source,
                )
            })?;
        }
        Ok(PayloadBlobRef {
            blob_id,
            media_type: media_type.to_owned(),
            path: path.to_string_lossy().to_string(),
            byte_count: content.len() as u64,
        })
    }
}

pub fn build_observability_bundle(
    input: &HarnessInput,
    output: &HarnessOutput,
    invocations: &[RecordedModelInvocation],
    events: &[CapturedEvent],
    blob_store: &BlobStore,
) -> Result<ObservabilityBundle, AppError> {
    let ledger = &output.turn_ledger;
    if ledger.entries.len() != invocations.len() {
        return Err(AppError::new(
            ErrorCode::SchemaValidationFailed,
            "turn ledger and invocation capture count must match",
            ErrorContext {
                component: "observability",
                operation: "build_bundle",
            },
        ));
    }

    let mut turn_captures = Vec::new();
    let mut event_records = Vec::new();
    let mut logs = Vec::new();

    for captured_event in events {
        let event = &captured_event.event;
        event_records.push(ObservabilityEventRecord {
            event_id: format!("event-{}", event_records.len()),
            captured_at: captured_event.captured_at.clone(),
            run_id: input.run_id.clone(),
            task_id: input.task_id.clone(),
            fixture_id: input.fixture_id.clone(),
            strategy_id: input.config.strategy.strategy_id.clone(),
            turn_index: event_turn_index(event),
            component: event_component(event).to_owned(),
            event_type: event_name(event).to_owned(),
            details: serde_json::to_value(event).unwrap_or_else(|_| json!({})),
            blob_refs: Vec::new(),
        });
        logs.push(json!({
            "captured_at": captured_event.captured_at,
            "run_id": input.run_id,
            "task_id": input.task_id,
            "turn_index": event_turn_index(event),
            "strategy_id": input.config.strategy.strategy_id.clone(),
            "fixture_id": input.fixture_id,
            "component": event_component(event),
            "event": event_name(event),
            "error_code": event_error_code(event),
        }));
    }

    for (entry, invocation) in ledger.entries.iter().zip(invocations.iter()) {
        let turn_index = entry.turn_trace.turn_index;
        let request_blob = blob_store.write_text(
            "blobs",
            "application/json",
            &serde_json::to_string_pretty(&invocation.raw_request).unwrap_or_default(),
        )?;
        let raw_response_blob = blob_store.write_text(
            "blobs",
            "application/json",
            &serde_json::to_string_pretty(&invocation.raw_response).unwrap_or_default(),
        )?;
        let validated_response_blob = blob_store.write_text(
            "blobs",
            "application/json",
            &serde_json::to_string_pretty(&invocation.response).unwrap_or_default(),
        )?;
        let prompt_blob = blob_store.write_text("blobs", "text/plain", &entry.rendered_prompt)?;
        let context_blob = blob_store.write_text("blobs", "text/plain", &entry.rendered_context)?;

        turn_captures.push(TurnTelemetryCapture {
            turn_index,
            request_blob: request_blob.clone(),
            raw_response_blob: raw_response_blob.clone(),
            validated_response_blob: validated_response_blob.clone(),
            prompt_blob: prompt_blob.clone(),
            context_blob: context_blob.clone(),
            provider_request_id: invocation.provider_request_id.clone(),
            telemetry: entry.turn_trace.telemetry.clone(),
            tool_traces: entry.tool_traces.clone(),
        });

        event_records.push(ObservabilityEventRecord {
            event_id: format!("event-{}", event_records.len()),
            captured_at: turn_completed_timestamp(events, turn_index)
                .unwrap_or("1970-01-01T00:00:00Z")
                .to_owned(),
            run_id: input.run_id.clone(),
            task_id: input.task_id.clone(),
            fixture_id: input.fixture_id.clone(),
            strategy_id: input.config.strategy.strategy_id.clone(),
            turn_index: Some(turn_index),
            component: "observability".to_owned(),
            event_type: "turn.payloads_captured".to_owned(),
            details: json!({
                "provider_request_id": invocation.provider_request_id,
                "tool_calls": entry.tool_traces.len(),
            }),
            blob_refs: vec![
                request_blob,
                raw_response_blob,
                validated_response_blob,
                prompt_blob,
                context_blob,
            ],
        });
        logs.push(json!({
            "captured_at": turn_completed_timestamp(events, turn_index).unwrap_or("1970-01-01T00:00:00Z"),
            "run_id": input.run_id,
            "task_id": input.task_id,
            "turn_index": turn_index,
            "strategy_id": input.config.strategy.strategy_id.clone(),
            "fixture_id": input.fixture_id,
            "component": "observability",
            "event": "turn.payloads_captured",
            "error_code": Value::Null,
        }));
    }

    let aggregation = RunTelemetryAggregation {
        total_turns: ledger.entries.len() as u32,
        aggregate_prompt_bytes: ledger
            .entries
            .iter()
            .map(|entry| u64::from(entry.turn_trace.telemetry.prompt_bytes))
            .sum(),
        aggregate_prompt_tokens: ledger
            .entries
            .iter()
            .map(|entry| u64::from(entry.turn_trace.telemetry.prompt_tokens))
            .sum(),
        aggregate_latency_ms: ledger
            .entries
            .iter()
            .map(|entry| u64::from(entry.turn_trace.telemetry.latency_ms))
            .sum(),
        aggregate_tool_calls: ledger
            .entries
            .iter()
            .map(|entry| u64::from(entry.turn_trace.telemetry.tool_calls))
            .sum(),
    };

    let run_manifest = RunManifest {
        run_id: input.run_id.clone(),
        schema_version: graphbench_core::artifacts::RUN_MANIFEST_SCHEMA_VERSION,
        fixture_id: input.fixture_id.clone(),
        task_id: input.task_id.clone(),
        strategy_id: input.config.strategy.strategy_id.clone(),
        strategy_config: input.config.strategy.clone(),
        harness_version: input.config.harness_version.clone(),
        schema_version_set: RunSchemaVersionSet {
            fixture_manifest: graphbench_core::artifacts::FIXTURE_MANIFEST_SCHEMA_VERSION,
            task_spec: graphbench_core::artifacts::TASK_SPEC_SCHEMA_VERSION,
            evidence_spec: graphbench_core::artifacts::EVIDENCE_SPEC_SCHEMA_VERSION,
            strategy_config: graphbench_core::STRATEGY_CONFIG_SCHEMA_VERSION,
            context_object: graphbench_core::artifacts::CONTEXT_OBJECT_SCHEMA_VERSION,
            context_window_section:
                graphbench_core::artifacts::CONTEXT_WINDOW_SECTION_SCHEMA_VERSION,
            turn_trace: TURN_TRACE_SCHEMA_VERSION,
            score_report: graphbench_core::artifacts::SCORE_REPORT_SCHEMA_VERSION,
        },
        provider: invocations
            .last()
            .map(|invocation| invocation.response.provider.clone())
            .unwrap_or_default(),
        model_slug: invocations
            .last()
            .map(|invocation| invocation.response.model_slug.clone())
            .unwrap_or_default(),
        prompt_version: input.config.prompt_version.clone(),
        graph_snapshot_id: input.graph_prompt.context_hash.clone(),
        started_at: events
            .iter()
            .map(|event| event.captured_at.as_str())
            .next()
            .unwrap_or("1970-01-01T00:00:00Z")
            .to_owned(),
        completed_at: events
            .iter()
            .rev()
            .map(|event| event.captured_at.as_str())
            .next()
            .unwrap_or("1970-01-01T00:00:00Z")
            .to_owned(),
        outcome: format!("{:?}", output.final_state).to_lowercase(),
    };

    event_records.sort_by(|left, right| {
        left.captured_at
            .cmp(&right.captured_at)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    logs.sort_by(|left, right| {
        left["captured_at"]
            .as_str()
            .cmp(&right["captured_at"].as_str())
            .then_with(|| left["event"].as_str().cmp(&right["event"].as_str()))
    });

    Ok(ObservabilityBundle {
        run_manifest,
        events: event_records,
        turns: turn_captures,
        aggregation,
        structured_logs: logs,
    })
}

#[derive(Debug, Clone)]
pub struct RecordedModelInvocation {
    pub response: HarnessModelResponse,
    pub raw_request: Value,
    pub raw_response: Value,
    pub provider_request_id: Option<String>,
}

impl From<&ModelInvocation> for RecordedModelInvocation {
    fn from(value: &ModelInvocation) -> Self {
        Self {
            response: value.response.clone(),
            raw_request: value.raw_request.clone(),
            raw_response: value.raw_response.clone(),
            provider_request_id: value.provider_request_id.clone(),
        }
    }
}

fn event_name(event: &HarnessEvent) -> &'static str {
    match event {
        HarnessEvent::RunStarted { .. } => "run.started",
        HarnessEvent::TurnStarted { .. } => "turn.started",
        HarnessEvent::PromptAssembled { .. } => "prompt.assembled",
        HarnessEvent::ModelRequestSent { .. } => "model.request_sent",
        HarnessEvent::ModelResponseReceived { .. } => "model.response_received",
        HarnessEvent::ModelResponseRejected { .. } => "model.response_rejected",
        HarnessEvent::ModelResponseValidated { .. } => "model.response_validated",
        HarnessEvent::ToolStarted { .. } => "tool.started",
        HarnessEvent::ToolRequested { .. } => "tool.requested",
        HarnessEvent::ToolCompleted { .. } => "tool.completed",
        HarnessEvent::ToolFailed { .. } => "tool.failed",
        HarnessEvent::GraphSessionMutated { .. } => "graph_session.mutated",
        HarnessEvent::ReadinessChanged { .. } => "readiness.changed",
        HarnessEvent::EvidenceMatched { .. } => "evidence.matched",
        HarnessEvent::RunCompleted { .. } => "run.completed",
        HarnessEvent::RunFailed { .. } => "run.failed",
    }
}

fn event_component(event: &HarnessEvent) -> &'static str {
    match event {
        HarnessEvent::ToolRequested { .. }
        | HarnessEvent::ToolStarted { .. }
        | HarnessEvent::ToolCompleted { .. }
        | HarnessEvent::ToolFailed { .. } => "tool",
        HarnessEvent::ModelRequestSent { .. }
        | HarnessEvent::ModelResponseReceived { .. }
        | HarnessEvent::ModelResponseRejected { .. }
        | HarnessEvent::ModelResponseValidated { .. } => "provider",
        HarnessEvent::GraphSessionMutated { .. } => "graph",
        HarnessEvent::ReadinessChanged { .. } | HarnessEvent::EvidenceMatched { .. } => "scoring",
        HarnessEvent::RunStarted { .. }
        | HarnessEvent::TurnStarted { .. }
        | HarnessEvent::PromptAssembled { .. }
        | HarnessEvent::RunCompleted { .. }
        | HarnessEvent::RunFailed { .. } => "harness",
    }
}

fn event_turn_index(event: &HarnessEvent) -> Option<u32> {
    match event {
        HarnessEvent::TurnStarted { turn_index, .. }
        | HarnessEvent::PromptAssembled { turn_index, .. }
        | HarnessEvent::ModelRequestSent { turn_index }
        | HarnessEvent::ModelResponseReceived { turn_index, .. }
        | HarnessEvent::ModelResponseRejected { turn_index, .. }
        | HarnessEvent::ModelResponseValidated { turn_index, .. }
        | HarnessEvent::ToolRequested { turn_index, .. }
        | HarnessEvent::ToolStarted { turn_index, .. }
        | HarnessEvent::ToolCompleted { turn_index, .. }
        | HarnessEvent::ToolFailed { turn_index, .. }
        | HarnessEvent::ReadinessChanged { turn_index, .. }
        | HarnessEvent::EvidenceMatched { turn_index, .. } => Some(*turn_index),
        HarnessEvent::GraphSessionMutated { .. } => None,
        HarnessEvent::RunStarted { .. }
        | HarnessEvent::RunCompleted { .. }
        | HarnessEvent::RunFailed { .. } => None,
    }
}

fn event_error_code(event: &HarnessEvent) -> Value {
    match event {
        HarnessEvent::ModelResponseRejected { error, .. }
        | HarnessEvent::ToolFailed { error, .. }
        | HarnessEvent::RunFailed { error, .. } => Value::String(error.clone()),
        _ => Value::Null,
    }
}

fn turn_completed_timestamp(events: &[CapturedEvent], turn_index: u32) -> Option<&str> {
    events.iter().find_map(|captured| match &captured.event {
        HarnessEvent::ReadinessChanged {
            turn_index: event_turn_index,
            ..
        } if *event_turn_index == turn_index => Some(captured.captured_at.as_str()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BlobStore, CapturedEvent, ObservabilityBundle, PayloadBlobRef, RunTelemetryAggregation,
    };
    use crate::strategy::graph_then_targeted_lexical_read;
    use graphbench_core::artifacts::{RunManifest, RunSchemaVersionSet};

    #[test]
    fn blob_store_writes_hash_addressed_payloads() {
        let root = std::env::temp_dir().join("graphbench-observability-test");
        let store = BlobStore::new(&root).expect("blob store");
        let blob = store
            .write_text("blobs", "application/json", "{\"ok\":true}")
            .expect("blob");
        assert!(root.join("blobs").exists());
        assert!(blob.blob_id.starts_with("sha256:"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_observability_bundle_fails_validation() {
        let bundle = ObservabilityBundle {
            run_manifest: RunManifest {
                run_id: "run-1".to_owned(),
                schema_version: graphbench_core::artifacts::RUN_MANIFEST_SCHEMA_VERSION,
                fixture_id: "fixture-1".to_owned(),
                task_id: "task-1".to_owned(),
                strategy_id: "graph.targeted-lexical-read".to_owned(),
                strategy_config: graph_then_targeted_lexical_read(),
                harness_version: "0.1.0".to_owned(),
                schema_version_set: RunSchemaVersionSet {
                    fixture_manifest: 1,
                    task_spec: 1,
                    evidence_spec: 1,
                    strategy_config: 1,
                    context_object: 1,
                    context_window_section: 1,
                    turn_trace: 1,
                    score_report: 1,
                },
                provider: "openrouter".to_owned(),
                model_slug: "model".to_owned(),
                prompt_version: "v1".to_owned(),
                graph_snapshot_id:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
                started_at: "1970-01-01T00:00:00Z".to_owned(),
                completed_at: "1970-01-01T00:00:00Z".to_owned(),
                outcome: "done".to_owned(),
            },
            events: vec![super::ObservabilityEventRecord {
                event_id: "event-1".to_owned(),
                captured_at: "1970-01-01T00:00:00Z".to_owned(),
                run_id: "run-2".to_owned(),
                task_id: "task-1".to_owned(),
                fixture_id: "fixture-1".to_owned(),
                strategy_id: "graph.targeted-lexical-read".to_owned(),
                turn_index: None,
                component: "harness".to_owned(),
                event_type: "run.started".to_owned(),
                details: serde_json::json!({}),
                blob_refs: vec![PayloadBlobRef {
                    blob_id:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_owned(),
                    media_type: "text/plain".to_owned(),
                    path: "missing.txt".to_owned(),
                    byte_count: 0,
                }],
            }],
            turns: vec![],
            aggregation: RunTelemetryAggregation {
                total_turns: 0,
                aggregate_prompt_bytes: 0,
                aggregate_prompt_tokens: 0,
                aggregate_latency_ms: 0,
                aggregate_tool_calls: 0,
            },
            structured_logs: Vec::new(),
        };

        assert!(bundle.validate(std::env::temp_dir()).is_err());
    }

    #[test]
    fn captured_event_serializes_stably() {
        let captured = CapturedEvent {
            captured_at: "1970-01-01T00:00:00Z".to_owned(),
            event: crate::runtime::HarnessEvent::RunStarted {
                run_id: "run-1".to_owned(),
            },
        };
        let value = serde_json::to_value(&captured).expect("captured event");
        assert_eq!(value["captured_at"], "1970-01-01T00:00:00Z");
    }

    #[test]
    fn aggregation_turn_mismatch_fails_validation() {
        let temp_root = std::env::temp_dir().join("graphbench-observability-mismatch");
        std::fs::create_dir_all(&temp_root).expect("temp root");
        let bundle = ObservabilityBundle {
            run_manifest: RunManifest {
                run_id: "run-1".to_owned(),
                schema_version: graphbench_core::artifacts::RUN_MANIFEST_SCHEMA_VERSION,
                fixture_id: "fixture-1".to_owned(),
                task_id: "task-1".to_owned(),
                strategy_id: "graph.targeted-lexical-read".to_owned(),
                strategy_config: graph_then_targeted_lexical_read(),
                harness_version: "0.1.0".to_owned(),
                schema_version_set: RunSchemaVersionSet {
                    fixture_manifest: 1,
                    task_spec: 1,
                    evidence_spec: 1,
                    strategy_config: 1,
                    context_object: 1,
                    context_window_section: 1,
                    turn_trace: 1,
                    score_report: 1,
                },
                provider: "mock-provider".to_owned(),
                model_slug: "mock-model".to_owned(),
                prompt_version: "v1".to_owned(),
                graph_snapshot_id:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
                started_at: "1970-01-01T00:00:00Z".to_owned(),
                completed_at: "1970-01-01T00:00:01Z".to_owned(),
                outcome: "done".to_owned(),
            },
            events: vec![super::ObservabilityEventRecord {
                event_id: "event-1".to_owned(),
                captured_at: "1970-01-01T00:00:00Z".to_owned(),
                run_id: "run-1".to_owned(),
                task_id: "task-1".to_owned(),
                fixture_id: "fixture-1".to_owned(),
                strategy_id: "graph.targeted-lexical-read".to_owned(),
                turn_index: None,
                component: "harness".to_owned(),
                event_type: "run.started".to_owned(),
                details: serde_json::json!({}),
                blob_refs: Vec::new(),
            }],
            turns: vec![super::TurnTelemetryCapture {
                turn_index: 0,
                request_blob: PayloadBlobRef {
                    blob_id:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_owned(),
                    media_type: "application/json".to_owned(),
                    path: temp_root.join("request.json").to_string_lossy().to_string(),
                    byte_count: 2,
                },
                raw_response_blob: PayloadBlobRef {
                    blob_id:
                        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                            .to_owned(),
                    media_type: "application/json".to_owned(),
                    path: temp_root
                        .join("response.json")
                        .to_string_lossy()
                        .to_string(),
                    byte_count: 2,
                },
                validated_response_blob: PayloadBlobRef {
                    blob_id:
                        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                            .to_owned(),
                    media_type: "application/json".to_owned(),
                    path: temp_root
                        .join("validated.json")
                        .to_string_lossy()
                        .to_string(),
                    byte_count: 2,
                },
                prompt_blob: PayloadBlobRef {
                    blob_id:
                        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                            .to_owned(),
                    media_type: "text/plain".to_owned(),
                    path: temp_root.join("prompt.txt").to_string_lossy().to_string(),
                    byte_count: 2,
                },
                context_blob: PayloadBlobRef {
                    blob_id:
                        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
                            .to_owned(),
                    media_type: "text/plain".to_owned(),
                    path: temp_root.join("context.txt").to_string_lossy().to_string(),
                    byte_count: 2,
                },
                provider_request_id: None,
                telemetry: graphbench_core::artifacts::TelemetryCounts {
                    prompt_bytes: 2,
                    prompt_tokens: 1,
                    latency_ms: 1,
                    tool_calls: 0,
                },
                tool_traces: Vec::new(),
            }],
            aggregation: RunTelemetryAggregation {
                total_turns: 2,
                aggregate_prompt_bytes: 2,
                aggregate_prompt_tokens: 1,
                aggregate_latency_ms: 1,
                aggregate_tool_calls: 0,
            },
            structured_logs: Vec::new(),
        };
        for name in [
            "request.json",
            "response.json",
            "validated.json",
            "prompt.txt",
            "context.txt",
        ] {
            std::fs::write(temp_root.join(name), "{}").expect("blob fixture");
        }
        assert!(bundle.validate(&temp_root).is_err());
        let _ = std::fs::remove_dir_all(temp_root);
    }
}
