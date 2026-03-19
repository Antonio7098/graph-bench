import { useState, useEffect, useRef } from "react";
import type { ReactElement } from "react";
import { apiClient, type RunDetail as RunDetailType, type TurnTrace, type RunEvent, type RenderedSection } from "../api/client";
import { Button } from "./Button";
import { JsonToggle } from "./JsonToggle";

interface RunDetailProps {
  runId: string;
  onBack: () => void;
  detail?: RunDetailType;
}

type Tab = "overview" | "timeline" | "evidence" | "graph" | "omissions" | "stream";

interface ModalState {
  isOpen: boolean;
  title: string;
  content?: string;
  jsonData?: unknown;
}

interface GraphViewProps {
  sessionJson: string;
  turnIndex: number;
}

function GraphView({ sessionJson, turnIndex }: GraphViewProps): ReactElement {
  const [selectedNode, setSelectedNode] = useState<string | null>(null);
  const [showDependencies, setShowDependencies] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 800, height: 500 });

  interface Node {
    id: string;
    label: string;
    detailLevel: string;
    pinned: boolean;
    origin: { kind: string };
    hydratedSource?: { path: string; snippet?: string };
  }

  interface Edge {
    source: string;
    target: string;
    relation: string;
    isDependency: boolean;
  }

  function parseGraphSession(json: string): { nodes: Node[]; edges: Edge[] } {
    try {
      const session = JSON.parse(json);
      const nodes: Node[] = [];
      const edges: Edge[] = [];
      const nodeMap = new Map<string, Node>();

      if (session.context?.selected) {
        for (const [id, data] of Object.entries(session.context.selected as Record<string, unknown>)) {
          const blockData = data as Record<string, unknown>;
          const node: Node = {
            id,
            label: (blockData.hydrated_source as { path?: string })?.path?.split('/').pop() || id.slice(0, 8),
            detailLevel: (blockData.detail_level as string) || 'unknown',
            pinned: (blockData.pinned as boolean) || false,
            origin: (blockData.origin as { kind: string }) || { kind: 'unknown' },
            hydratedSource: blockData.hydrated_source as { path: string; snippet?: string } | undefined,
          };
          nodes.push(node);
          nodeMap.set(id, node);
        }
      }

      if (session.context?.relations) {
        for (const rel of session.context.relations as Array<{ from: string; to: string; kind: string }>) {
          if (nodeMap.has(rel.from) && nodeMap.has(rel.to)) {
            edges.push({
              source: rel.from,
              target: rel.to,
              relation: rel.kind,
              isDependency: rel.kind === 'depends_on' || rel.kind === 'dependency_of',
            });
          }
        }
      }

      return { nodes, edges };
    } catch {
      return { nodes: [], edges: [] };
    }
  }

  function computeLayout(nodes: Node[], edges: Edge[]): Map<string, { x: number; y: number }> {
    const positions = new Map<string, { x: number; y: number }>();
    if (nodes.length === 0) return positions;

    const structuralEdges = edges.filter(e => !e.isDependency);
    const dependencyEdges = edges.filter(e => e.isDependency);

    const nodeIndices = new Map(nodes.map((n, i) => [n.id, i]));
    const adj = new Map<string, Set<string>>();
    nodes.forEach(n => adj.set(n.id, new Set()));
    structuralEdges.forEach(e => {
      adj.get(e.source)?.add(e.target);
      adj.get(e.target)?.add(e.source);
    });

    const layers: string[][] = [];
    const placed = new Set<string>();
    const roots = nodes.filter(n => {
      const incoming = structuralEdges.filter(e => e.target === n.id).length;
      return incoming === 0;
    });

    if (roots.length === 0 && nodes.length > 0) {
      const firstNode = nodes[0];
      if (firstNode) roots.push(firstNode);
    }

    function assignLayer(nodeId: string, layer: number) {
      if (placed.has(nodeId)) return;
      placed.add(nodeId);
      if (!layers[layer]) layers[layer] = [];
      layers[layer].push(nodeId);
      
      const neighbors = adj.get(nodeId) || new Set();
      for (const neighbor of neighbors) {
        if (!placed.has(neighbor)) {
          assignLayer(neighbor, layer + 1);
        }
      }
    }

    roots.forEach(r => assignLayer(r.id, 0));

    const remaining = nodes.filter(n => !placed.has(n.id));
    let layerIdx = layers.length;
    while (remaining.length > 0) {
      const layer: string[] = [];
      layers[layerIdx] = layer;
      const batchSize = Math.ceil(remaining.length / 2);
      for (let i = 0; i < batchSize && remaining.length > 0; i++) {
        const node = remaining.shift();
        if (node) {
          layer.push(node.id);
          placed.add(node.id);
        }
      }
      layerIdx++;
    }

    const layerWidth = dimensions.width / (Math.max(...layers.map(l => l.length), 1));
    const layerHeight = dimensions.height / (Math.max(layers.length, 1));

    layers.forEach((layer, layerIdx) => {
      layer.forEach((nodeId, nodeIdx) => {
        const x = (nodeIdx + 0.5) * layerWidth;
        const y = (layerIdx + 0.5) * layerHeight;
        positions.set(nodeId, { x, y });
      });
    });

    const dependencyOffset = 60;
    dependencyEdges.forEach(edge => {
      const sourcePos = positions.get(edge.source);
      const targetPos = positions.get(edge.target);
      if (sourcePos && targetPos) {
        const midX = (sourcePos.x + targetPos.x) / 2;
        const midY = (sourcePos.y + targetPos.y) / 2;
        const perpX = -(targetPos.y - sourcePos.y);
        const perpY = targetPos.x - sourcePos.x;
        const len = Math.sqrt(perpX * perpX + perpY * perpY) || 1;
        positions.set(`${edge.source}->${edge.target}`, {
          x: midX + (perpX / len) * dependencyOffset,
          y: midY + (perpY / len) * dependencyOffset,
        });
      }
    });

    return positions;
  }

  const { nodes, edges } = parseGraphSession(sessionJson);
  const positions = computeLayout(nodes, edges);
  const structuralEdges = edges.filter(e => !e.isDependency);
  const dependencyEdges = edges.filter(e => e.isDependency);

  const getNodeColor = (node: Node): string => {
    if (node.pinned) return "var(--color-accent-amber)";
    if (node.detailLevel === "source") return "var(--color-accent-cyan)";
    if (node.detailLevel === "neighborhood") return "var(--color-accent-green)";
    return "var(--color-text-secondary)";
  };

  return (
    <div className="graph-view" ref={containerRef} style={{ position: "relative", width: "100%", height: "500px", background: "var(--color-bg-deep)", borderRadius: "var(--radius-md)", overflow: "hidden" }}>
      <svg width={dimensions.width} height={dimensions.height} style={{ display: "block" }}>
        <defs>
          <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
            <polygon points="0 0, 10 3.5, 0 7" fill="var(--color-text-muted)" />
          </marker>
          <marker id="dep-arrow" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
            <polygon points="0 0, 10 3.5, 0 7" fill="var(--color-accent-magenta)" />
          </marker>
        </defs>

        {showDependencies && dependencyEdges.map((edge, i) => {
          const source = positions.get(edge.source);
          const target = positions.get(edge.target);
          const labelPos = positions.get(`${edge.source}->${edge.target}`);
          if (!source || !target || !labelPos) return null;
          return (
            <g key={`dep-${i}`}>
              <line x1={source.x} y1={source.y} x2={target.x} y2={target.y} stroke="var(--color-accent-magenta)" strokeWidth="1.5" strokeDasharray="4,2" opacity="0.5" markerEnd="url(#dep-arrow)" />
              <text x={labelPos.x} y={labelPos.y} fill="var(--color-accent-magenta)" fontSize="9" textAnchor="middle">{edge.relation}</text>
            </g>
          );
        })}

        {structuralEdges.map((edge, i) => {
          const source = positions.get(edge.source);
          const target = positions.get(edge.target);
          if (!source || !target) return null;
          return (
            <line key={`struct-${i}`} x1={source.x} y1={source.y} x2={target.x} y2={target.y} stroke="var(--color-border)" strokeWidth="2" markerEnd="url(#arrowhead)" />
          );
        })}

        {nodes.map(node => {
          const pos = positions.get(node.id);
          if (!pos) return null;
          const isSelected = selectedNode === node.id;
          return (
            <g key={node.id} transform={`translate(${pos.x}, ${pos.y})`} style={{ cursor: "pointer" }} onClick={() => setSelectedNode(isSelected ? null : node.id)}>
              <circle r={isSelected ? 28 : 22} fill="var(--color-bg-elevated)" stroke={getNodeColor(node)} strokeWidth={isSelected ? 3 : 2} />
              <text textAnchor="middle" dy="4" fill="var(--color-text-primary)" fontSize="10" fontFamily="var(--font-mono)">{node.label.slice(0, 10)}</text>
            </g>
          );
        })}
      </svg>

      {selectedNode && (
        <div style={{ position: "absolute", bottom: "1rem", left: "1rem", right: "1rem", background: "var(--color-bg-surface)", border: "1px solid var(--color-border)", borderRadius: "var(--radius-md)", padding: "0.75rem", fontSize: "0.8rem" }}>
          <div style={{ fontWeight: 600, color: "var(--color-accent-cyan)", marginBottom: "0.25rem" }}>{selectedNode}</div>
          <div>Detail: {nodes.find(n => n.id === selectedNode)?.detailLevel || "unknown"}</div>
          <div>Origin: {nodes.find(n => n.id === selectedNode)?.origin.kind || "unknown"}</div>
          {nodes.find(n => n.id === selectedNode)?.hydratedSource?.path && (
            <div style={{ marginTop: "0.5rem", color: "var(--color-text-muted)", fontSize: "0.7rem", fontFamily: "var(--font-mono)" }}>
              {nodes.find(n => n.id === selectedNode)?.hydratedSource?.path}
            </div>
          )}
        </div>
      )}

      <div style={{ position: "absolute", top: "0.75rem", right: "0.75rem", display: "flex", gap: "0.5rem" }}>
        <button className="modal-btn" onClick={() => setShowDependencies(!showDependencies)}>
          {showDependencies ? "🔗 Hide Deps" : "🔗 Show Deps"}
        </button>
      </div>

      <div style={{ position: "absolute", bottom: "0.5rem", right: "0.75rem", fontSize: "0.65rem", color: "var(--color-text-muted)" }}>
        {nodes.length} nodes, {structuralEdges.length} structural edges, {dependencyEdges.length} dependencies
      </div>
    </div>
  );
}

interface PromptModalContentProps {
  data: { sections: RenderedSection[] | undefined };
}

function PromptModalContent({ data }: PromptModalContentProps): ReactElement {
  const [viewMode, setViewMode] = useState<"text" | "rendered" | "json">("text");

  const sections = data.sections;
  const fullText = sections
    ? sections.map(section => `=== ${section.title} ===\n\n${section.content}`).join("\n\n")
    : "No prompt sections available.";

  return (
    <div className="json-toggle-container" style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
      <div className="detail-header">
        <span className="json-toggle-label">{sections?.length ?? 0} section{(sections?.length ?? 0) !== 1 ? "s" : ""}</span>
        <div className="view-toggle">
          <button className={viewMode === "text" ? "active" : ""} onClick={() => setViewMode("text")}>Text</button>
          <button className={viewMode === "rendered" ? "active" : ""} onClick={() => setViewMode("rendered")}>Rendered</button>
          <button className={viewMode === "json" ? "active" : ""} onClick={() => setViewMode("json")}>JSON</button>
        </div>
      </div>
      <div style={{ flex: 1, overflow: "auto", minHeight: 0 }}>
        {viewMode === "text" && (
          <pre className="modal-pre" style={{ whiteSpace: "pre-wrap", wordBreak: "break-word", margin: 0 }}>{fullText}</pre>
        )}
        {viewMode === "rendered" && sections && (
          <div className="rendered-view">
            {sections.map((section, idx) => (
              <div key={idx} className="rendered-array-item" style={{ marginBottom: "1rem", padding: "0.75rem", background: "var(--color-bg-secondary)", borderRadius: "var(--radius-sm)" }}>
                <div className="rendered-field">
                  <label className="rendered-label">section_id</label>
                  <span className="rendered-string">{section.section_id}</span>
                </div>
                <div className="rendered-field">
                  <label className="rendered-label">title</label>
                  <span className="rendered-string">{section.title}</span>
                </div>
                <div className="rendered-field">
                  <label className="rendered-label">content</label>
                  <div className="rendered-string-block" style={{ maxHeight: "200px", overflow: "auto", whiteSpace: "pre-wrap", wordBreak: "break-word" }}>{section.content}</div>
                </div>
                <div className="rendered-field">
                  <label className="rendered-label">byte_count</label>
                  <span className="rendered-number">{section.byte_count}</span>
                </div>
                <div className="rendered-field">
                  <label className="rendered-label">token_count</label>
                  <span className="rendered-number">{section.token_count}</span>
                </div>
                <div className="rendered-field">
                  <label className="rendered-label">schema_version</label>
                  <span className="rendered-number">{section.schema_version}</span>
                </div>
              </div>
            ))}
          </div>
        )}
        {viewMode === "json" && (
          <JsonToggle data={sections || []} />
        )}
      </div>
    </div>
  );
}

export function RunDetail({ runId, onBack, detail: propDetail }: RunDetailProps): ReactElement {
  const [data, setData] = useState<RunDetailType | null>(propDetail || null);
  const [loading, setLoading] = useState(!propDetail);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<Tab>("overview");
  const [selectedTurnIndex, setSelectedTurnIndex] = useState<number | null>(null);
  const [modal, setModal] = useState<ModalState>({ isOpen: false, title: "", content: "" });
  const [showGraphView, setShowGraphView] = useState(false);
  const [graphTurnIndex, setGraphTurnIndex] = useState<number | null>(null);
  const isDemo = !!propDetail;
  
  // Live streaming state
  const [streamEvents, setStreamEvents] = useState<RunEvent[]>([]);
  const [runStatus, setRunStatus] = useState<"running" | "completed" | "failed" | null>(null);
  const streamEventsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isDemo) {
      loadRunDetail();
    }
  }, [runId, isDemo]);

  // Auto-switch to Live Logs tab when run is running
  useEffect(() => {
    if (runStatus === "running" && activeTab === "overview") {
      setActiveTab("stream");
    }
  }, [runStatus, activeTab]);

  // Poll for events and status during run
  useEffect(() => {
    if (isDemo) return;

    let intervalId: ReturnType<typeof setInterval>;
    let lastSeq = 0;
    let lastTurnsLoad = 0;
    const isRunning = { current: true };
    let hasLoadedStatus = false;

    async function pollEvents() {
      if (!isRunning.current) return;

      try {
        const events = await apiClient.getRunEvents(runId, lastSeq || undefined);
        if (!isRunning.current) return;

        if (events.length > 0) {
          setStreamEvents(prev => {
            const existingSeqs = new Set(prev.map(e => e.seq));
            const newEvents = events.filter(e => !existingSeqs.has(e.seq));
            return [...prev, ...newEvents];
          });
          const latestEvent = events[events.length - 1];
          if (latestEvent) {
            lastSeq = latestEvent.seq;
            if (latestEvent.event_type === "run.completed" || latestEvent.event_type === "run.failed") {
              isRunning.current = false;
              setRunStatus(latestEvent.event_type === "run.completed" ? "completed" : "failed");
              loadRunDetail();
            } else {
              if (!hasLoadedStatus) {
                hasLoadedStatus = true;
                setRunStatus("running");
              }
              // Reload turns data periodically (every 5 seconds)
              const now = Date.now();
              if (now - lastTurnsLoad > 5000) {
                lastTurnsLoad = now;
                loadRunDetail();
              }
            }
          }
        } else if (!hasLoadedStatus) {
          const runs = await apiClient.listRuns();
          const thisRun = runs.find(r => r.run_id === runId);
          if (thisRun?.status && isRunning.current) {
            if (thisRun.status === "completed" || thisRun.status === "failed") {
              isRunning.current = false;
              hasLoadedStatus = true;
              setRunStatus(thisRun.status as "completed" | "failed");
              loadRunDetail();
            } else {
              hasLoadedStatus = true;
              setRunStatus("running");
            }
          }
        }

        if (isRunning.current) {
          streamEventsRef.current?.scrollIntoView({ behavior: "smooth" });
        }
      } catch {
        // Silently ignore polling errors
      }
    }

    pollEvents();
    intervalId = setInterval(pollEvents, 2000);

    return () => {
      isRunning.current = false;
      if (intervalId) clearInterval(intervalId);
    };
  }, [runId, isDemo]);

  async function loadRunDetail() {
    setLoading(true);
    setError(null);
    try {
      const result = await apiClient.getRun(runId);
      setData(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load run");
    } finally {
      setLoading(false);
    }
  }

  function openModal(title: string, content?: string, jsonData?: unknown) {
    setModal({ isOpen: true, title, content, jsonData });
  }

  function openTurnFromEvent(turnIndex: number | null | undefined) {
    if (turnIndex === null || turnIndex === undefined) return;
    const turn = turns.find(t => t.turn_index === turnIndex);
    if (!turn) {
      setModal({ isOpen: true, title: `Turn ${turnIndex}`, content: "Turn data not yet available. The turn may still be in progress." });
      return;
    }
    setModal({ isOpen: true, title: `Turn ${turn.turn_index}`, content: "", jsonData: turn });
  }

  function openToolEventDetails(event: RunEvent) {
    if (!event.tool_name) return;
    const details = event.details || {};
    const toolData = {
      tool_name: event.tool_name,
      turn_index: event.turn_index,
      component: event.component,
      event_type: event.event_type,
      message: event.message,
      captured_at: event.captured_at,
      ...details,
    };
    setModal({ isOpen: true, title: `Tool: ${event.tool_name}`, content: "", jsonData: toolData });
  }

  async function openStrategyModal() {
    const strategyId = manifest.strategy_id;
    setModal({ isOpen: true, title: `Strategy: ${strategyId}`, content: "Loading..." });
    try {
      const strategy = await apiClient.getStrategy(strategyId);
      setModal({ 
        isOpen: true, 
        title: `Strategy: ${strategyId}`, 
        content: "",
        jsonData: strategy,
      });
    } catch (e) {
      setModal({ 
        isOpen: true, 
        title: `Strategy: ${strategyId}`, 
        content: `Strategy ID: ${strategyId}\n\n(Strategy configuration not found)` 
      });
    }
  }

  async function openTaskModal() {
    const taskId = manifest.task_id;
    setModal({ isOpen: true, title: `Task: ${taskId}`, content: "Loading..." });
    try {
      const task = await apiClient.getTask(taskId);
      setModal({ 
        isOpen: true, 
        title: `Task: ${taskId}`, 
        content: "",
        jsonData: task,
      });
    } catch (e) {
      setModal({ 
        isOpen: true, 
        title: `Task: ${taskId}`, 
        content: `Task ID: ${taskId}\n\n(Task details not found)` 
      });
    }
  }

  function closeModal() {
    setModal({ isOpen: false, title: "", content: "" });
  }

  function getFullPromptText(sections: RenderedSection[] | undefined): string {
    if (!sections || sections.length === 0) {
      return "No prompt sections available for this turn.";
    }
    return sections
      .map(section => `=== ${section.title} ===\n\n${section.content}`)
      .join("\n\n");
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  function getScoreClass(score: number): string {
    if (score >= 0.8) return "excellent";
    if (score >= 0.5) return "good";
    return "poor";
  }

  if (loading) {
    return (
      <>
        <header className="content-header">
          <Button variant="ghost" onClick={onBack}>← Back</Button>
        </header>
        <div className="content-body">
          <div className="loading"><div className="spinner" /></div>
        </div>
      </>
    );
  }

  if (error || !data) {
    return (
      <>
        <header className="content-header">
          <Button variant="ghost" onClick={onBack}>← Back</Button>
        </header>
        <div className="content-body">
          <div className="empty-state">
            <div className="empty-icon">⚠️</div>
            <div className="empty-title">Error</div>
            <div className="empty-description">{error}</div>
          </div>
        </div>
      </>
    );
  }

  const { manifest, turns, evidence_matches, score_report } = data;

  return (
    <>
      <header className="content-header">
        <button className="nav-item" onClick={onBack}>← Back</button>
        <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
          <span className="run-id">{runId}</span>
          <span className="strategy-badge">{manifest.strategy_id}</span>
        </div>
      </header>

      <div className="tabs">
        {runStatus === "running" && (
          <button 
            className={`tab ${activeTab === "stream" ? "active" : ""}`}
            onClick={() => setActiveTab("stream")}
            style={{ color: "#10b981", fontWeight: "bold" }}
          >
            Live Logs {streamEvents.length > 0 && `(${streamEvents.length})`}
          </button>
        )}
        <button 
          className={`tab ${activeTab === "overview" ? "active" : ""}`}
          onClick={() => setActiveTab("overview")}
        >
          Overview
        </button>
        <button 
          className={`tab ${activeTab === "timeline" ? "active" : ""}`}
          onClick={() => setActiveTab("timeline")}
        >
          Turn Timeline
        </button>
        <button 
          className={`tab ${activeTab === "evidence" ? "active" : ""}`}
          onClick={() => setActiveTab("evidence")}
        >
          Evidence ({evidence_matches.length})
        </button>
        <button 
          className={`tab ${activeTab === "graph" ? "active" : ""}`}
          onClick={() => { setActiveTab("graph"); setGraphTurnIndex(turns[0]?.turn_index ?? 0); }}
        >
          Graph State
        </button>
        <button 
          className={`tab ${activeTab === "omissions" ? "active" : ""}`}
          onClick={() => setActiveTab("omissions")}
        >
          Omissions
        </button>
      </div>

      <div className="content-body">
        {activeTab === "stream" && (
          <div className="fade-in">
            <div className="card" style={{ marginBottom: "1rem" }}>
              <div className="card-header">
                <h3 className="card-title">
                  Live Event Stream 
                  {runStatus === "running" && <span style={{ color: "#10b981", marginLeft: "0.5rem" }}>● Running</span>}
                  {runStatus === "completed" && <span style={{ color: "#3b82f6", marginLeft: "0.5rem" }}>● Completed</span>}
                  {runStatus === "failed" && <span style={{ color: "#ef4444", marginLeft: "0.5rem" }}>● Failed</span>}
                </h3>
              </div>
              <div className="card-body">
                <div style={{ display: "flex", gap: "2rem", marginBottom: "1rem", padding: "0.75rem", background: "var(--color-bg-secondary)", borderRadius: "8px" }}>
                  <div>
                    <div className="section-title">Started</div>
                    <div>{new Date(manifest.started_at).toLocaleString()}</div>
                  </div>
                  <div>
                    <div className="section-title">Duration</div>
                    <div>{manifest.completed_at ? 
                      `${Math.round((new Date(manifest.completed_at).getTime() - new Date(manifest.started_at).getTime()) / 1000)}s` :
                      `${Math.round((Date.now() - new Date(manifest.started_at).getTime()) / 1000)}s (ongoing)`
                    }</div>
                  </div>
                  <div>
                    <div className="section-title">Turns</div>
                    <div>{Math.max(...streamEvents.map(e => e.turn_index ?? -1), data?.turns.length ?? 0 - 1) + 1}</div>
                  </div>
                  <div>
                    <div className="section-title">Tool Calls</div>
                    <div>{streamEvents.filter(e => e.event_type === "tool.completed").length}</div>
                  </div>
                  <div>
                    <div className="section-title">Events</div>
                    <div>{streamEvents.length}</div>
                  </div>
                </div>
                
                <div style={{ maxHeight: "60vh", overflowY: "auto", background: "var(--color-bg-secondary)", borderRadius: "8px", padding: "0.75rem", fontFamily: "monospace", fontSize: "0.8rem" }}>
                  {streamEvents.length === 0 ? (
                    <div style={{ color: "var(--color-text-muted)", textAlign: "center", padding: "2rem" }}>Waiting for events...</div>
                  ) : (
                    streamEvents.slice(-100).map((event, idx) => (
                      <div 
                        key={idx} 
                        style={{ 
                          padding: "0.25rem 0", 
                          borderBottom: "1px solid var(--color-border)", 
                          color: event.level === "error" ? "#ef4444" : event.event_type?.includes("completed") ? "#10b981" : event.event_type?.includes("started") ? "#3b82f6" : "var(--color-text)",
                          cursor: "pointer",
                        }}
                        onClick={() => {
                          if (event.event_type?.startsWith("tool.")) {
                            openToolEventDetails(event);
                          } else {
                            openTurnFromEvent(event.turn_index);
                          }
                        }}
                      >
                        <span style={{ color: "var(--color-text-muted)", marginRight: "0.5rem" }}>[{new Date(event.captured_at).toLocaleTimeString()}]</span>
                        <span style={{ color: "#8b5cf6", marginRight: "0.5rem" }}>{event.component}</span>
                        <span style={{ fontWeight: "bold", marginRight: "0.5rem" }}>{event.event_type}</span>
                        {event.turn_index !== undefined && event.turn_index !== null && <span style={{ color: "#f59e0b", marginRight: "0.5rem" }}>[turn {event.turn_index}]</span>}
                        {event.tool_name && <span style={{ color: "#06b6d4", marginRight: "0.5rem" }}>{event.tool_name}</span>}
                        <span>{event.message}</span>
                      </div>
                    ))
                  )}
                  <div ref={streamEventsRef} />
                </div>
              </div>
            </div>
          </div>
        )}
        
        {activeTab === "overview" && (
          <div className="fade-in">
            <div className="card" style={{ marginBottom: "1.5rem" }}>
              <div className="card-header">
                <h3 className="card-title">Run Metadata</h3>
              </div>
              <div className="card-body">
                <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "1rem" }}>
                  <div>
                    <div className="section-title">Status</div>
                    <div className={`run-outcome ${manifest.outcome || runStatus || "unknown"}`}>
                      {manifest.outcome || runStatus || "unknown"}
                    </div>
                  </div>
                  <div>
                    <div className="section-title"> Fixture</div>
                    <div>{manifest.fixture_id}</div>
                  </div>
                  <div>
                    <div className="section-title">Task</div>
                    <div 
                      style={{ cursor: "pointer", color: "var(--color-accent-cyan)" }}
                      onClick={() => openTaskModal()}
                    >
                      {manifest.task_id}
                    </div>
                  </div>
                  <div>
                    <div className="section-title">Strategy</div>
                    <div 
                      style={{ cursor: "pointer", color: "var(--color-accent-cyan)" }}
                      onClick={() => openStrategyModal()}
                    >
                      {manifest.strategy_id}
                    </div>
                  </div>
                  <div>
                    <div className="section-title">Provider</div>
                    <div>{manifest.provider} / {manifest.model_slug}</div>
                  </div>
                  <div>
                    <div className="section-title">Harness Version</div>
                    <div>{manifest.harness_version}</div>
                  </div>
                  <div>
                    <div className="section-title">Started</div>
                    <div>{formatDate(manifest.started_at)}</div>
                  </div>
                  <div>
                    <div className="section-title">Completed</div>
                    <div>{formatDate(manifest.completed_at)}</div>
                  </div>
                </div>
              </div>
            </div>

            {score_report && (
              <div className="card">
                <div className="card-header">
                  <h3 className="card-title">Score Report</h3>
                </div>
                <div className="card-body">
                  <div className="score-grid">
                    <div className="score-card">
                      <div className="score-label">Visibility</div>
                      <div className={`score-value ${getScoreClass(score_report.evidence_visibility_score)}`}>
                        {(score_report.evidence_visibility_score * 100).toFixed(1)}%
                      </div>
                    </div>
                    <div className="score-card">
                      <div className="score-label">Acquisition</div>
                      <div className={`score-value ${getScoreClass(score_report.evidence_acquisition_score)}`}>
                        {(score_report.evidence_acquisition_score * 100).toFixed(1)}%
                      </div>
                    </div>
                    <div className="score-card">
                      <div className="score-label">Efficiency</div>
                      <div className={`score-value ${getScoreClass(score_report.evidence_efficiency_score)}`}>
                        {(score_report.evidence_efficiency_score * 100).toFixed(1)}%
                      </div>
                    </div>
                    <div className="score-card">
                      <div className="score-label">Explanation</div>
                      <div className={`score-value ${getScoreClass(score_report.explanation_quality_score)}`}>
                        {(score_report.explanation_quality_score * 100).toFixed(1)}%
                      </div>
                    </div>
                  </div>
                  
                  {score_report.metrics && (
                    <div style={{ marginTop: "1.5rem" }}>
                      <div className="section-title">Metrics</div>
                      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "1rem", marginTop: "0.75rem" }}>
                        <div className="metric-row" style={{ flexDirection: "column", gap: "0.25rem" }}>
                          <span className="metric-label">Evidence Recall</span>
                          <span className="metric-value">{((score_report.metrics.required_evidence_recall ?? 0) * 100).toFixed(1)}%</span>
                        </div>
                        <div className="metric-row" style={{ flexDirection: "column", gap: "0.25rem" }}>
                          <span className="metric-label">Evidence Precision</span>
                          <span className="metric-value">{((score_report.metrics.evidence_precision ?? 0) * 100).toFixed(1)}%</span>
                        </div>
                        <div className="metric-row" style={{ flexDirection: "column", gap: "0.25rem" }}>
                          <span className="metric-label">Irrelevant Material</span>
                          <span className="metric-value">{((score_report.metrics.irrelevant_material_ratio ?? 0) * 100).toFixed(1)}%</span>
                        </div>
                        <div className="metric-row" style={{ flexDirection: "column", gap: "0.25rem" }}>
                          <span className="metric-label">Turns to Ready</span>
                          <span className="metric-value">{score_report.metrics.turns_to_readiness ?? "N/A"}</span>
                        </div>
                        <div className="metric-row" style={{ flexDirection: "column", gap: "0.25rem" }}>
                          <span className="metric-label">Reread Count</span>
                          <span className="metric-value">{score_report.metrics.reread_count}</span>
                        </div>
                        <div className="metric-row" style={{ flexDirection: "column", gap: "0.25rem" }}>
                          <span className="metric-label">Post-Ready Drift</span>
                          <span className="metric-value">{score_report.metrics.post_readiness_drift_turns}</span>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        )}

        {activeTab === "timeline" && (
          <div className="turn-timeline fade-in">
            {turns.map((turn) => (
              <div 
                key={turn.turn_index} 
                className={`turn-item ${selectedTurnIndex === turn.turn_index ? "turn-selected" : ""}`}
                onClick={() => setSelectedTurnIndex(selectedTurnIndex === turn.turn_index ? null : turn.turn_index)}
                style={{ cursor: "pointer" }}
              >
                <div className="turn-header">
                  <span className="turn-index">
                    {selectedTurnIndex === turn.turn_index ? "▼" : "▶"} Turn {turn.turn_index}
                  </span>
                  <span className={`turn-state ${(turn.readiness_state || 'unknown').replace("_", "-")}`}>
                    {(turn.readiness_state || 'unknown').replace("_", " ")}
                  </span>
                </div>
                <div className="turn-telemetry">
                  <div className="telemetry-item">
                    <span>Tokens:</span>
                    <span className="telemetry-value">{turn.telemetry.prompt_tokens}</span>
                  </div>
                  <div className="telemetry-item">
                    <span>Latency:</span>
                    <span className="telemetry-value">{turn.telemetry.latency_ms}ms</span>
                  </div>
                  <div className="telemetry-item">
                    <span>Tools:</span>
                    <span className="telemetry-value">{turn.telemetry.tool_calls}</span>
                  </div>
                  <div className="telemetry-item">
                    <span>Evidence Δ:</span>
                    <span className="telemetry-value">{(turn.evidence_delta || []).length}</span>
                  </div>
                </div>
                {turn.readiness_reason && (
                  <div style={{ marginTop: "0.5rem", fontSize: "0.8rem", color: "var(--color-text-muted)" }}>
                    {turn.readiness_reason}
                  </div>
                )}
                
                {selectedTurnIndex === turn.turn_index && (
                  <div style={{ marginTop: "1rem", paddingTop: "1rem", borderTop: "1px solid var(--color-border-muted)" }}>
                    <div style={{ display: "flex", gap: "0.75rem", marginBottom: "1rem" }}>
                      <button 
                        className="modal-btn"
                        onClick={(e) => { e.stopPropagation(); openModal(`Full Prompt - Turn ${turn.turn_index}`, "", { sections: turn.selection?.rendered_sections }); }}
                      >
                        📝 View Full Prompt
                      </button>
                      {turn.tool_calls && turn.tool_calls.length > 0 && (
                        <button 
                          className="modal-btn"
                          onClick={(e) => { e.stopPropagation(); openModal(`Tool Calls - Turn ${turn.turn_index}`, "", turn.tool_calls); }}
                        >
                          🔧 View Tool Calls ({turn.tool_calls.length})
                        </button>
                      )}
                      {turn.graph_session_after && (
                        <button 
                          className="modal-btn"
                          onClick={(e) => { 
                            e.stopPropagation(); 
                            setGraphTurnIndex(turn.turn_index);
                            setShowGraphView(true);
                            setModal({ isOpen: true, title: `Graph State - Turn ${turn.turn_index}`, content: "" });
                          }}
                        >
                          🕸️ Graph State
                        </button>
                      )}
                    </div>
                    
                    {turn.request && (
                      <>
                        <div className="section-title" style={{ marginBottom: "0.75rem" }}>Request</div>
                        <div className="section-content" style={{ marginBottom: "1rem" }}>
                          <div><strong>Prompt Version:</strong> {turn.request.prompt_version ?? "N/A"}</div>
                          <div><strong>Prompt Hash:</strong> <code style={{ fontSize: "0.7rem" }}>{turn.request.prompt_hash ?? "N/A"}</code></div>
                          <div><strong>Context Hash:</strong> <code style={{ fontSize: "0.7rem" }}>{turn.request.context_hash ?? "N/A"}</code></div>
                        </div>
                      </>
                    )}
                    
                    {turn.response && (
                      <>
                        <div className="section-title" style={{ marginBottom: "0.75rem" }}>Response</div>
                        <div className="section-content" style={{ marginBottom: "1rem" }}>
                          <div><strong>Provider:</strong> {turn.response.provider ?? "N/A"}</div>
                          <div><strong>Model:</strong> {turn.response.model_slug ?? "N/A"}</div>
                          <div><strong>Validated:</strong> {turn.response.validated ? "✓ Yes" : "✗ No"}</div>
                        </div>
                      </>
                    )}
                    
                    {turn.selection && (
                      <>
                        <div className="section-title" style={{ marginBottom: "0.75rem" }}>Selection</div>
                        <div className="section-content" style={{ marginBottom: "1rem" }}>
                          <div><strong>Selected Context Objects:</strong></div>
                          <ul style={{ margin: "0.5rem 0", paddingLeft: "1.25rem" }}>
                            {(turn.selection.selected_context_objects ?? []).map((ctx, i) => (
                              <li key={i} style={{ fontFamily: "var(--font-mono)", fontSize: "0.8rem" }}>{ctx}</li>
                            ))}
                          </ul>
                          {(turn.selection.omitted_candidates ?? []).length > 0 && (
                            <>
                              <div style={{ marginTop: "0.75rem" }}><strong>Omitted Candidates:</strong></div>
                              <ul style={{ margin: "0.5rem 0", paddingLeft: "1.25rem" }}>
                                {turn.selection.omitted_candidates.map((omission, i) => (
                                  <li key={i} style={{ fontSize: "0.8rem", color: "var(--color-accent-red)" }}>
                                    {omission.candidate_id} - {omission.reason}
                                  </li>
                                ))}
                              </ul>
                            </>
                          )}
                        </div>
                      </>
                    )}
                    
                    {turn.telemetry && (
                      <>
                        <div className="section-title" style={{ marginBottom: "0.75rem" }}>Telemetry</div>
                        <div className="section-content" style={{ marginBottom: "1rem" }}>
                          <div><strong>Prompt Bytes:</strong> {turn.telemetry.prompt_bytes ?? 0}</div>
                          <div><strong>Prompt Tokens:</strong> {turn.telemetry.prompt_tokens ?? 0}</div>
                          <div><strong>Latency:</strong> {turn.telemetry.latency_ms ?? 0}ms</div>
                          <div><strong>Tool Calls:</strong> {turn.telemetry.tool_calls ?? 0}</div>
                        </div>
                      </>
                    )}
                    
                    {(turn.evidence_delta ?? []).length > 0 && (
                      <>
                        <div className="section-title" style={{ marginBottom: "0.75rem" }}>Evidence Delta</div>
                        <div className="section-content">
                          {turn.evidence_delta.map((fact, i) => (
                            <div key={i} style={{ color: "var(--color-accent-magenta)" }}>{fact}</div>
                          ))}
                        </div>
                      </>
                    )}
                    
                    {turn.hashes && (
                      <>
                        <div className="section-title" style={{ marginBottom: "0.75rem", marginTop: "1rem" }}>Hashes</div>
                        <div className="section-content">
                          <div><strong>Turn Hash:</strong></div>
                          <code style={{ fontSize: "0.65rem", wordBreak: "break-all" }}>{turn.hashes.turn_hash ?? "N/A"}</code>
                        </div>
                      </>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {activeTab === "evidence" && (
          <div className="evidence-timeline fade-in">
            {evidence_matches.length === 0 ? (
              <div className="empty-state">
                <div className="empty-icon">🔍</div>
                <div className="empty-title">No Evidence Matched</div>
                <div className="empty-description">No evidence facts were matched during this run.</div>
              </div>
            ) : (
              evidence_matches.map((match, idx) => (
                <div key={`${match.turn_index}-${match.fact_id}-${idx}`} className="evidence-item">
                  <div className="evidence-dot" />
                  <span className="evidence-fact-id">{match.fact_id}</span>
                  <span style={{ color: "var(--color-text-muted)" }}>at turn {match.turn_index}</span>
                  <span style={{ marginLeft: "auto", fontSize: "0.75rem", color: "var(--color-text-muted)" }}>
                    {formatDate(match.matched_at)}
                  </span>
                </div>
              ))
            )}
          </div>
        )}

        {activeTab === "omissions" && (
          <div className="fade-in">
            {turns.map((turn) => {
              const omissions = turn.selection?.omitted_candidates ?? [];
              if (omissions.length === 0) return null;
              return (
                <div key={turn.turn_index} className="context-section">
                  <div className="section-title">Turn {turn.turn_index}</div>
                  <div className="omission-list">
                    {omissions.map((omission, idx) => (
                      <div key={idx} className="omission-item">
                        <span className="omission-candidate">{omission.candidate_id}</span>
                        <span className="omission-reason">{omission.reason}</span>
                      </div>
                    ))}
                  </div>
                </div>
              );
            })}
            {turns.every(t => (t.selection?.omitted_candidates ?? []).length === 0) && (
              <div className="empty-state">
                <div className="empty-icon">✅</div>
                <div className="empty-title">No Omissions</div>
                <div className="empty-description">No context objects were omitted during this run.</div>
              </div>
            )}
          </div>
        )}

        {activeTab === "graph" && (
          <div className="graph-tab fade-in">
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "1rem", padding: "0.75rem", background: "var(--color-bg-surface)", borderRadius: "var(--radius-md)" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                <button 
                  className="modal-btn"
                  onClick={() => setGraphTurnIndex(prev => Math.max((turns[0]?.turn_index ?? 0), (prev ?? turns[0]?.turn_index ?? 0) - 1))}
                  disabled={graphTurnIndex !== null && graphTurnIndex <= (turns[0]?.turn_index ?? 0)}
                >
                  ← Previous
                </button>
                <span style={{ fontWeight: 600, minWidth: "120px", textAlign: "center" }}>
                  Turn {graphTurnIndex ?? turns[0]?.turn_index ?? 0}
                </span>
                <button 
                  className="modal-btn"
                  onClick={() => setGraphTurnIndex(prev => Math.min((turns[turns.length - 1]?.turn_index ?? 0), (prev ?? turns[0]?.turn_index ?? 0) + 1))}
                  disabled={graphTurnIndex !== null && graphTurnIndex >= (turns[turns.length - 1]?.turn_index ?? 0)}
                >
                  Next →
                </button>
              </div>
              <div style={{ fontSize: "0.8rem", color: "var(--color-text-muted)" }}>
                {turns.filter(t => t.graph_session_after).length} turns with graph state
              </div>
            </div>

            {(() => {
              const currentTurn = turns.find(t => t.turn_index === graphTurnIndex) || turns[0];
              if (!currentTurn?.graph_session_after) {
                return (
                  <div className="empty-state">
                    <div className="empty-icon">🕸️</div>
                    <div className="empty-title">No Graph State</div>
                    <div className="empty-description">This turn does not have graph state data.</div>
                  </div>
                );
              }

              return (
                <div style={{ display: "grid", gridTemplateColumns: "1fr 350px", gap: "1rem" }}>
                  <div>
                    <GraphView 
                      sessionJson={currentTurn.graph_session_after} 
                      turnIndex={currentTurn.turn_index} 
                    />
                  </div>
                  <div style={{ background: "var(--color-bg-surface)", borderRadius: "var(--radius-md)", padding: "1rem", maxHeight: "500px", overflow: "auto" }}>
                    <div className="section-title" style={{ marginBottom: "0.75rem" }}>Tool Calls Leading to This State</div>
                    {currentTurn.tool_calls && currentTurn.tool_calls.length > 0 ? (
                      currentTurn.tool_calls.map((tc, idx) => (
                        <div key={idx} style={{ marginBottom: "1rem", padding: "0.75rem", background: "var(--color-bg-elevated)", borderRadius: "var(--radius-sm)", borderLeft: "3px solid var(--color-accent-cyan)" }}>
                          <div style={{ fontWeight: 600, fontSize: "0.85rem", color: "var(--color-accent-cyan)" }}>{tc.tool_name}</div>
                          <pre style={{ margin: "0.5rem 0 0", fontSize: "0.7rem", fontFamily: "var(--font-mono)", whiteSpace: "pre-wrap", wordBreak: "break-all", maxHeight: "120px", overflow: "auto" }}>
                            {JSON.stringify(tc.payload, null, 2)}
                          </pre>
                        </div>
                      ))
                    ) : (
                      <div style={{ color: "var(--color-text-muted)", fontSize: "0.85rem" }}>No tool calls for this turn.</div>
                    )}
                  </div>
                </div>
              );
            })()}
          </div>
        )}
      </div>

      {modal.isOpen && (
        <div className="modal-overlay" onClick={closeModal}>
          <div className="modal-content" onClick={e => e.stopPropagation()} style={{ maxWidth: showGraphView ? "95vw" : "900px" }}>
            <div className="modal-header">
              <h3 className="modal-title">{modal.title}</h3>
              <button className="modal-close" onClick={() => { closeModal(); setShowGraphView(false); }}>×</button>
            </div>
            {showGraphView && graphTurnIndex !== null ? (
              <div style={{ flex: 1, overflow: "auto", padding: "1rem" }}>
                <GraphView 
                  sessionJson={turns.find(t => t.turn_index === graphTurnIndex)?.graph_session_after || "{}"} 
                  turnIndex={graphTurnIndex} 
                />
              </div>
            ) : (
              <div className="modal-body" style={{ flex: 1, overflow: "auto" }}>
                {modal.title.startsWith("Full Prompt") && modal.jsonData ? (
                  <PromptModalContent data={modal.jsonData as { sections: RenderedSection[] | undefined }} />
                ) : modal.jsonData ? (
                  <JsonToggle data={modal.jsonData} />
                ) : (
                  <pre className="modal-pre">{modal.content}</pre>
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
