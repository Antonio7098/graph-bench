import { useState, useEffect, useRef, useCallback } from "react";
import type { ReactElement } from "react";
import { apiClient, type RunSummary, type RunFilter } from "../api/client";
import { mockRuns } from "../api/mock";

interface RunListProps {
  onSelectRun: (runId: string) => void;
  onRunComplete?: (runId: string) => void;
  runs?: RunSummary[];
}

export function RunList({ onSelectRun, onRunComplete, runs: propRuns }: RunListProps): ReactElement {
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [loading, setLoading] = useState(!propRuns);
  const [filter, setFilter] = useState<RunFilter>({});
  const [error, setError] = useState<string | null>(null);
  const [showRunModal, setShowRunModal] = useState(false);
  const isDemo = !!propRuns;

  useEffect(() => {
    if (!isDemo) {
      loadRuns();
    } else {
      setRuns(propRuns);
    }
  }, [filter, isDemo, propRuns]);

  async function loadRuns() {
    setLoading(true);
    setError(null);
    try {
      const data = await apiClient.listRuns(Object.keys(filter).length > 0 ? filter : undefined);
      setRuns(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load runs");
    } finally {
      setLoading(false);
    }
  }

  function applyFilters(allRuns: RunSummary[]): RunSummary[] {
    return allRuns.filter(run => {
      if (filter.fixture_id && !run.fixture_id.includes(filter.fixture_id)) return false;
      if (filter.task_id && !run.task_id.includes(filter.task_id)) return false;
      if (filter.strategy_id && !run.strategy_id.includes(filter.strategy_id)) return false;
      if (filter.outcome && run.outcome !== filter.outcome) return false;
      return true;
    });
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  const displayRuns = applyFilters(runs);

  return (
    <>
      <header className="content-header">
        <h2 className="content-title">Runs</h2>
        <button className="btn-primary" onClick={() => setShowRunModal(true)}>
          Run Benchmark
        </button>
      </header>
      
      <div className="filter-bar">
        <input
          type="text"
          className="filter-input"
          placeholder="Filter by fixture..."
          value={filter.fixture_id || ""}
          onChange={(e) => setFilter(f => ({ ...f, fixture_id: e.target.value || undefined }))}
        />
        <input
          type="text"
          className="filter-input"
          placeholder="Filter by task..."
          value={filter.task_id || ""}
          onChange={(e) => setFilter(f => ({ ...f, task_id: e.target.value || undefined }))}
        />
        <input
          type="text"
          className="filter-input"
          placeholder="Filter by strategy..."
          value={filter.strategy_id || ""}
          onChange={(e) => setFilter(f => ({ ...f, strategy_id: e.target.value || undefined }))}
        />
        <select
          className="filter-select"
          value={filter.outcome || ""}
          onChange={(e) => setFilter(f => ({ ...f, outcome: e.target.value || undefined }))}
        >
          <option value="">All Outcomes</option>
          <option value="success">Success</option>
          <option value="failure">Failure</option>
        </select>
      </div>

      <div className="content-body">
        {loading && (
          <div className="loading">
            <div className="spinner" />
          </div>
        )}

        {error && (
          <div className="empty-state">
            <div className="empty-icon">⚠️</div>
            <div className="empty-title">Error Loading Runs</div>
            <div className="empty-description">{error}</div>
          </div>
        )}

        {!loading && !error && displayRuns.length === 0 && (
          <div className="empty-state">
            <div className="empty-icon">📭</div>
            <div className="empty-title">No Runs Found</div>
            <div className="empty-description">
              No benchmark runs match your filters. Try adjusting your search criteria.
            </div>
          </div>
        )}

        {!loading && !error && displayRuns.length > 0 && (
          <div className="run-list">
            {displayRuns.map((run, idx) => (
              <div 
                key={run.run_id} 
                className={`run-item fade-in stagger-${Math.min(idx + 1, 5)}`}
                onClick={() => onSelectRun(run.run_id)}
              >
                <div>
                  <div className="run-id">{run.run_id}</div>
                  <div className="run-meta">
                    <span className="run-strategy">{run.strategy_id}</span>
                    <span>{run.task_id}</span>
                    <span>{run.turn_count} turns</span>
                  </div>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                  <span className={`run-outcome ${run.outcome}`}>{run.outcome}</span>
                  <span style={{ fontSize: "0.75rem", color: "var(--color-text-muted)" }}>
                    {formatDate(run.started_at)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {showRunModal && <RunBenchmarkModal onClose={() => setShowRunModal(false)} onRunComplete={(runId) => {
        setShowRunModal(false);
        if (onRunComplete) {
          onRunComplete(runId);
        }
      }} />}
    </>
  );
}

interface RunBenchmarkModalProps {
  onClose: () => void;
  onRunComplete: (runId: string) => void;
}

interface LogLine {
  time: string;
  message: string;
  type: "info" | "error" | "success" | "event";
  data?: Record<string, unknown>;
}

interface StreamEvent {
  seq?: number;
  captured_at?: string;
  stream?: string;
  run_id?: string | null;
  event_type?: string;
  component?: string;
  level?: string;
  message?: string;
  turn_index?: number;
  tool_name?: string | null;
  metrics?: Record<string, unknown> | null;
  details?: Record<string, unknown>;
  [key: string]: unknown;
}

function RunBenchmarkModal({ onClose, onRunComplete }: RunBenchmarkModalProps): ReactElement {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [taskSpecPath, setTaskSpecPath] = useState("tasks/prepare-to-edit/task-01.task.json");
  const [fixturePath, setFixturePath] = useState("fixtures/graphbench-internal/fixture.json");
  const [modelId, setModelId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [strategy, setStrategy] = useState("graph_then_targeted_lexical_read");
  const [turnBudget, setTurnBudget] = useState("48");
  const [timeoutMs, setTimeoutMs] = useState("300000");
  const [tokenBudget, setTokenBudget] = useState("2000000");
  const [promptHeadroom, setPromptHeadroom] = useState("24576");
  const [seedOverview, setSeedOverview] = useState("2");
  const [initialSelect, setInitialSelect] = useState("crates/graphbench-core/src/artifacts.rs");
  const [representationLevel, setRepresentationLevel] = useState("L1");
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [status, setStatus] = useState<"idle" | "running" | "complete" | "error">("idle");
  const [currentRunId, setCurrentRunId] = useState<string | null>(null);
  const [availableStrategies, setAvailableStrategies] = useState<string[]>([]);
  const [availableTasks, setAvailableTasks] = useState<string[]>([]);
  const [availableFixtures, setAvailableFixtures] = useState<string[]>([]);
  const [loadingOptions, setLoadingOptions] = useState(true);
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

  useEffect(() => {
    async function loadOptions() {
      try {
        const [strategies, tasks, fixtures] = await Promise.all([
          apiClient.listStrategies(),
          apiClient.listTasks(),
          apiClient.listFixtures(),
        ]);
        setAvailableStrategies(strategies);
        setAvailableTasks(tasks);
        setAvailableFixtures(fixtures);
      } catch (e) {
        console.error("Failed to load options:", e);
      } finally {
        setLoadingOptions(false);
      }
    }
    loadOptions();
  }, []);

  function addLog(message: string, type: LogLine["type"] = "info", data?: Record<string, unknown>) {
    const time = new Date().toLocaleTimeString();
    setLogs(prev => [...prev, { time, message, type, data }]);
  }

  function formatEvent(event: StreamEvent): string {
    const prefix = event.turn_index !== undefined ? `[turn ${event.turn_index}] ` : "";
    const metricBits = event.metrics
      ? Object.entries(event.metrics)
          .map(([key, value]) => `${key}=${String(value)}`)
          .join(" ")
      : "";
    const toolBit = event.tool_name ? ` tool=${event.tool_name}` : "";
    const message = event.message || `${event.component || "system"} ${event.event_type || "event"}`;
    return `${prefix}${message}${toolBit}${metricBits ? ` ${metricBits}` : ""}`;
  }

  const connectToWebSocket = useCallback((runId: string) => {
    const ws = new WebSocket(`ws://localhost:3001/ws?run_id=${encodeURIComponent(runId)}`);
    wsRef.current = ws;
    
    ws.onopen = () => {
      addLog('Connected to live events', 'info');
    };
    
    ws.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data) as StreamEvent;
        
        if (event.run_id && event.run_id !== runId) {
          return;
        }

        if (event.event_type === 'run.completed') {
          setStatus("complete");
          addLog(formatEvent(event), 'success', event as Record<string, unknown>);
          setTimeout(() => {
            onRunComplete(runId);
          }, 1500);
          ws.close();
          return;
        }

        if (event.event_type === 'run.failed') {
          setStatus("error");
          addLog(formatEvent(event), 'error', event as Record<string, unknown>);
          return;
        }
        
        const message = formatEvent(event);
        addLog(message, event.level === 'error' ? 'error' : 'event', event as Record<string, unknown>);
        
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
  }, [onRunComplete]);

  async function handleRun() {
    setLoading(true);
    setError(null);
    setLogs([]);
    setStatus("running");
    addLog(`Starting benchmark run...`, "info");
    addLog(`Task: ${taskSpecPath}`, "info");
    addLog(`Fixture: ${fixturePath}`, "info");
    addLog(`Strategy: ${strategy}`, "info");
    if (modelId) addLog(`Model: ${modelId}`, "info");
    
    try {
      const response = await fetch(`http://localhost:3001/api/runs/run`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          task_spec_path: taskSpecPath,
          fixture_path: fixturePath,
          model_id: modelId || undefined,
          api_key: apiKey || undefined,
          strategy: strategy || undefined,
          turn_budget: turnBudget ? parseInt(turnBudget) : undefined,
          timeout_ms: timeoutMs ? parseInt(timeoutMs) : undefined,
          token_budget: tokenBudget ? parseInt(tokenBudget) : undefined,
          prompt_headroom: promptHeadroom ? parseInt(promptHeadroom) : undefined,
          seed_overview: seedOverview ? parseInt(seedOverview) : undefined,
          initial_select: initialSelect || undefined,
          representation_level: representationLevel || undefined,
        }),
      });
      
      if (!response.ok) {
        throw new Error(`Failed to start run: ${response.statusText}`);
      }
      
      const result = await response.json() as { success: boolean; run_id?: string; output?: string };
      
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
    onClose();
  }

  return (
    <div className="modal-overlay" onClick={handleCancel}>
      <div className="modal-content" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Run Benchmark</h3>
          <button className="modal-close" onClick={handleCancel}>&times;</button>
        </div>
        
        {status === "idle" && (
          <div className="modal-body">
            {loadingOptions ? (
              <div className="loading">
                <div className="spinner" />
              </div>
            ) : (
              <>
                <div className="form-group">
                  <label>Task</label>
                  <select
                    value={taskSpecPath}
                    onChange={(e) => setTaskSpecPath(e.target.value)}
                  >
                    {availableTasks.map(task => (
                      <option key={task} value={`tasks/${task}`}>{task}</option>
                    ))}
                  </select>
                </div>

                <div className="form-group">
                  <label>Fixture</label>
                  <select
                    value={fixturePath}
                    onChange={(e) => setFixturePath(e.target.value)}
                  >
                    {availableFixtures.map(fixture => (
                      <option key={fixture} value={`fixtures/${fixture}`}>{fixture}</option>
                    ))}
                  </select>
                </div>

                <div className="form-group">
                  <label>Strategy</label>
                  <select
                    value={strategy}
                    onChange={(e) => setStrategy(e.target.value)}
                  >
                    {availableStrategies.map(s => (
                      <option key={s} value={s}>{s}</option>
                    ))}
                  </select>
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

                <div className="form-group">
                  <label>API Key (optional)</label>
                  <input
                    type="password"
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="API key for model provider"
                  />
                </div>

                <div className="form-group">
                  <label>Turn Budget</label>
                  <input
                    type="number"
                    value={turnBudget}
                    onChange={(e) => setTurnBudget(e.target.value)}
                  />
                </div>

                <div className="form-group">
                  <label>Timeout (ms)</label>
                  <input
                    type="number"
                    value={timeoutMs}
                    onChange={(e) => setTimeoutMs(e.target.value)}
                  />
                </div>

                <div className="form-group">
                  <label>Token Budget</label>
                  <input
                    type="number"
                    value={tokenBudget}
                    onChange={(e) => setTokenBudget(e.target.value)}
                  />
                </div>

                <div className="form-group">
                  <label>Prompt Headroom</label>
                  <input
                    type="number"
                    value={promptHeadroom}
                    onChange={(e) => setPromptHeadroom(e.target.value)}
                  />
                </div>

                <div className="form-group">
                  <label>Seed Overview</label>
                  <input
                    type="number"
                    value={seedOverview}
                    onChange={(e) => setSeedOverview(e.target.value)}
                  />
                </div>

                <div className="form-group">
                  <label>Initial Select</label>
                  <input
                    type="text"
                    value={initialSelect}
                    onChange={(e) => setInitialSelect(e.target.value)}
                  />
                </div>

                <div className="form-group">
                  <label>Representation Level</label>
                  <input
                    type="text"
                    value={representationLevel}
                    onChange={(e) => setRepresentationLevel(e.target.value)}
                  />
                </div>
              </>
            )}
          </div>
        )}

        {logs.length > 0 && (
          <div className="modal-body">
            <div className="run-logs">
              {logs.map((log, idx) => (
                <div key={idx} className={`log-line log-${log.type}`}>
                  <span className="log-time">{log.time}</span>
                  <span className="log-message">{log.message}</span>
                </div>
              ))}
              <div ref={logsEndRef} />
            </div>
          </div>
        )}

        {error && <div className="error">{error}</div>}

        <div className="modal-footer">
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
    </div>
  );
}
