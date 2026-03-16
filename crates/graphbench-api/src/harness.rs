use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;
use anyhow::Result;

pub async fn run_benchmark(
    task_spec_path: &str,
    model_id: Option<&str>,
    event_tx: broadcast::Sender<String>,
) -> Result<(String, String)> {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--package")
        .arg("graphbench-harness")
        .arg("--bin")
        .arg("smoke_openrouter")
        .current_dir("/home/antonio/programming/Hivemind/graph-bench")
        .env("GRAPHBENCH_TASK_SPEC_PATH", task_spec_path);
    
    if let Some(model) = model_id {
        cmd.env("OPENROUTER_MODEL_ID", model);
    }
    
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let mut child = cmd.spawn()?;
    
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    
    let event_tx_clone = event_tx.clone();
    let stdout_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut output = String::new();
        
        while let Ok(Some(line)) = lines.next_line().await {
            output.push_str(&line);
            output.push('\n');
            
            let event = serde_json::json!({
                "type": "stdout",
                "component": "harness",
                "message": line,
            });
            let _ = event_tx_clone.send(event.to_string());
        }
        
        output
    });
    
    let event_tx_clone2 = event_tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut output = String::new();
        
        while let Ok(Some(line)) = lines.next_line().await {
            output.push_str(&line);
            output.push('\n');
            
            let event = serde_json::json!({
                "type": "stderr",
                "component": "harness",
                "message": line,
            });
            let _ = event_tx_clone2.send(event.to_string());
        }
        
        output
    });
    
    let status = child.wait().await?;
    
    let stdout_output = stdout_handle.await?;
    let stderr_output = stderr_handle.await?;
    
    let output = format!("{}\n{}", stdout_output, stderr_output);
    
    let run_id = if status.success() {
        output.lines()
            .find(|l| l.starts_with("run_id="))
            .map(|l| l.trim_start_matches("run_id=").to_string())
            .unwrap_or_else(|| format!("smoke-openrouter-{}", chrono::Utc::now().timestamp()))
    } else {
        return Err(anyhow::anyhow!("Run failed with status: {:?}", status));
    };
    
    let event = serde_json::json!({
        "type": "complete",
        "run_id": run_id,
    });
    let _ = event_tx.send(event.to_string());
    
    Ok((run_id, output))
}
