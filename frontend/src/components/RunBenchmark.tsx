import { useState, useEffect, useRef, useCallback } from "react";
import type { ReactElement } from "react";
import { apiClient } from "../api/client";

interface RunBenchmarkProps {
  onRunComplete: (runId: string) => void;
  onCancel: () => void;
}

interface LogLine {
  time: string;
  message: string;
  type: "info" | "error" | "success" | "event";
  data?: Record<string, unknown>;
}

interface StreamEvent {
  type?: string;
  event?: string;
  component?: string;
  turn_index?: number;
  [key: string]: unknown;
}

export function RunBenchmark({ onRunComplete, onCancel }: RunBenchmarkProps): ReactElement {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [taskSpecPath, setTaskSpecPath] = useState("tasks/prepare-to-edit/task-01.task.json");
  const [modelId, setModelId] = useState("");
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [status, setStatus] = useState<"idle" | "running" | "complete" | "error">("idle");
  const [currentRunId, setCurrentRunId] = useState<string | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  useEffect(() => {
    return () => {
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, []);

  function addLog(message: string, type: LogLine["type"] = "info", data?: Record<string, unknown>) {
    const time = new Date().toLocaleTimeString();
    setLogs(prev => [...prev, { time, message, type, data }]);
  }

  function formatEvent(event: StreamEvent): string {
    const { type: evtType, event: eventName, component, turn_index, ...rest } = event;
    
    const eventType = eventName || evtType || 'unknown';
    
    if (component === 'harness') {
      if (eventType === 'run.started') return 'Run started';
      if (eventType === 'run.completed') return 'Run completed';
      if (eventType === 'turn.started') return `Turn ${turn_index} started`;
      if (eventType === 'turn.completed') return `Turn ${turn_index} completed`;
    }
    
    if (component === 'provider') {
      if (eventType === 'model.request_sent') return 'Model request sent...';
      if (eventType === 'model.response_received') return 'Model response received';
    }
    
    if (component === 'tool') {
      if (eventType === 'tool.requested') return `Tool called`;
      if (eventType === 'tool.completed') return `Tool completed`;
    }
    
    if (component === 'graph') {
      if (eventType === 'graph_session.mutated') return 'Graph session mutated';
    }
    
    if (evtType === 'stdout' || evtType === 'stderr') {
      return (event.message as string)?.substring(0, 100) || eventType;
    }
    
    return `${component || 'system'}: ${eventType}`;
  }

  const connectToWebSocket = useCallback((runId: string) => {
    const ws = new WebSocket('ws://localhost:3001/ws');
    wsRef.current = ws;
    
    ws.onopen = () => {
      addLog('Connected to live events', 'info');
    };
    
    ws.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data) as StreamEvent;
        
        if (event.type === 'complete') {
          setStatus("complete");
          addLog('✓ Run completed!', 'success');
          setTimeout(() => {
            if (currentRunId) {
              onRunComplete(currentRunId);
            }
          }, 1500);
          ws.close();
          return;
        }
        
        const message = formatEvent(event);
        addLog(message, 'event', event as Record<string, unknown>);
        
        if (event.event === 'run.completed') {
          setStatus("complete");
          addLog('✓ Run completed!', 'success');
        }
        
      } catch (e) {
        console.error('WS parse error:', e);
      }
    };
    
    ws.onerror = () => {
      addLog('Connection error, waiting for completion...', 'info');
    };
    
    ws.onclose = () => {
      if (status !== 'complete') {
        addLog('Connection closed', 'info');
      }
    };
  }, [currentRunId, onRunComplete, status]);

  async function handleRun() {
    setLoading(true);
    setError(null);
    setLogs([]);
    setStatus("running");
    addLog(`Starting benchmark run...`, "info");
    addLog(`Task: ${taskSpecPath}`, "info");
    if (modelId) addLog(`Model: ${modelId}`, "info");
    
    try {
      const result = await apiClient.runBenchmark(taskSpecPath, modelId || undefined);
      
      if (result.success && result.run_id) {
        setCurrentRunId(result.run_id);
        addLog(`Run initialized: ${result.run_id}`, "info");
        
        addLog('Connecting to live events...', "info");
        connectToWebSocket(result.run_id);
      } else {
        addLog(`✗ Run failed`, "error");
        addLog(result.output || 'Unknown error', "error");
        setError("Run failed");
        setStatus("error");
      }
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : "Failed to run benchmark";
      addLog(`✗ Error: ${errMsg}`, "error");
      setError(errMsg);
      setStatus("error");
    } finally {
      setLoading(false);
    }
  }

  function handleCancel() {
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    addLog("Run cancelled", "info");
    setLoading(false);
    setStatus("idle");
    setCurrentRunId(null);
    onCancel();
  }

  return (
    <div className="run-benchmark">
      <h2>Run Benchmark</h2>
      
      {status === "idle" && (
        <>
          <div className="form-group">
            <label>Task Spec Path</label>
            <input
              type="text"
              value={taskSpecPath}
              onChange={(e) => setTaskSpecPath(e.target.value)}
              placeholder="tasks/prepare-to-edit/task-01.task.json"
            />
          </div>

          <div className="form-group">
            <label>Model ID (optional)</label>
            <input
              type="text"
              value={modelId}
              onChange={(e) => setModelId(e.target.value)}
              placeholder="Leave empty for default model"
            />
          </div>
        </>
      )}

      {logs.length > 0 && (
        <div className="run-logs">
          {logs.map((log, idx) => (
            <div key={idx} className={`log-line log-${log.type}`}>
              <span className="log-time">{log.time}</span>
              <span className="log-message">{log.message}</span>
            </div>
          ))}
          <div ref={logsEndRef} />
        </div>
      )}

      {error && <div className="error">{error}</div>}

      <div className="actions">
        {status !== "idle" && status !== "running" && (
          <button className="modal-btn" onClick={() => { setLogs([]); setStatus("idle"); }}>
            New Run
          </button>
        )}
        <button 
          className="modal-btn" 
          onClick={handleCancel}
          disabled={loading && status === "complete"}
        >
          {status === "complete" ? "Done" : status === "running" ? "Cancel" : "Cancel"}
        </button>
        {status === "idle" && (
          <button className="modal-btn primary" onClick={handleRun} disabled={loading}>
            Run
          </button>
        )}
      </div>
    </div>
  );
}
