use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde_json::json;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::info;

use crate::event_stream::{now_rfc3339, EventStream, StreamEvent};

#[derive(Debug)]
pub struct BenchmarkConfig {
    pub run_id: String,
    pub task_spec_path: String,
    pub fixture_path: String,
    pub model_id: Option<String>,
    pub api_key: Option<String>,
    pub strategy: String,
    pub turn_budget: u32,
    pub timeout_ms: u64,
    pub token_budget: u32,
    pub prompt_headroom: u32,
    pub seed_overview: u32,
    pub initial_select: String,
    pub representation_level: String,
}

pub async fn run_benchmark(
    config: BenchmarkConfig,
    event_stream: Arc<EventStream>,
) -> Result<(String, String)> {
    let run_id = config.run_id.clone();
    let workspace_root = workspace_root();

    publish_system_event(
        &event_stream,
        &run_id,
        "harness",
        "run.started",
        "info",
        "Starting benchmark subprocess",
        json!({
            "task_spec_path": config.task_spec_path,
            "fixture_path": config.fixture_path,
            "strategy": config.strategy,
            "workspace_root": workspace_root.display().to_string(),
        }),
    );

    let mut command = Command::new("cargo");
    command.current_dir(&workspace_root);
    command.arg("run");
    command.arg("--package");
    command.arg("graphbench-harness");
    command.arg("--bin");
    command.arg("smoke_openrouter");
    command.arg("--");
    command.arg("--run-id");
    command.arg(&config.run_id);
    command.arg("--task-spec");
    command.arg(&config.task_spec_path);
    command.arg("--fixture");
    command.arg(&config.fixture_path);
    command.arg("--strategy");
    command.arg(&config.strategy);
    command.arg("--turn-budget");
    command.arg(config.turn_budget.to_string());
    command.arg("--timeout-ms");
    command.arg(config.timeout_ms.to_string());
    command.arg("--token-budget");
    command.arg(config.token_budget.to_string());
    command.arg("--prompt-headroom");
    command.arg(config.prompt_headroom.to_string());
    command.arg("--seed-overview");
    command.arg(config.seed_overview.to_string());
    command.arg("--initial-select");
    command.arg(&config.initial_select);
    command.arg("--representation-level");
    command.arg(&config.representation_level);

    if let Some(model_id) = &config.model_id {
        command.arg("--model");
        command.arg(model_id);
    }

    if let Some(api_key) = &config.api_key {
        command.arg("--api-key");
        command.arg(api_key);
    }

    info!("[{}] Launching benchmark subprocess", run_id);
    publish_system_event(
        &event_stream,
        &run_id,
        "harness",
        "subprocess.spawned",
        "info",
        "Spawned smoke_openrouter subprocess",
        json!({
            "command": "cargo run --package graphbench-harness --bin smoke_openrouter -- ...",
            "timeout_ms": config.timeout_ms,
        }),
    );

    let outer_timeout = Duration::from_millis(config.timeout_ms.saturating_add(30_000));
    let output = match timeout(outer_timeout, command.output()).await {
        Ok(result) => result.context("Failed while waiting for benchmark subprocess")?,
        Err(_) => {
            publish_system_event(
                &event_stream,
                &run_id,
                "harness",
                "run.timeout",
                "error",
                "Benchmark subprocess timed out",
                json!({
                    "timeout_ms": outer_timeout.as_millis(),
                }),
            );
            return Err(anyhow!(
                "benchmark subprocess timed out after {} ms",
                outer_timeout.as_millis()
            ));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

    if !output.status.success() {
        publish_system_event(
            &event_stream,
            &run_id,
            "harness",
            "run.failed",
            "error",
            "Benchmark subprocess failed",
            json!({
                "status": output.status.code(),
                "stderr": stderr,
            }),
        );
        return Err(anyhow!(
            "benchmark subprocess failed with status {:?}: {}",
            output.status.code(),
            if stderr.is_empty() { stdout.clone() } else { stderr.clone() }
        ));
    }

    publish_system_event(
        &event_stream,
        &run_id,
        "harness",
        "run.completed",
        "info",
        "Benchmark subprocess completed",
        json!({
            "status": output.status.code(),
            "stdout_lines": stdout.lines().count(),
        }),
    );

    Ok((run_id, stdout))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn publish_system_event(
    event_stream: &EventStream,
    run_id: &str,
    component: &str,
    event_type: &str,
    level: &str,
    message: &str,
    details: serde_json::Value,
) {
    event_stream.publish(StreamEvent {
        seq: 0,
        captured_at: now_rfc3339(),
        stream: "live".to_owned(),
        run_id: Some(run_id.to_owned()),
        component: component.to_owned(),
        event_type: event_type.to_owned(),
        level: level.to_owned(),
        message: message.to_owned(),
        turn_index: None,
        tool_name: None,
        provider_request_id: None,
        metrics: None,
        tags: vec![component.to_owned()],
        details,
    });
}
