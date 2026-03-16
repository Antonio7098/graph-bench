import { useState, useMemo } from "react";
import type { ReactElement } from "react";
import type { RunSummary } from "../api/client";

interface LeaderboardProps {
  onSelectRun: (runId: string) => void;
  runs?: RunSummary[];
}

type GroupField = "harness_version" | "model_slug" | "strategy_id" | "provider" | "task_id";

interface GroupNode {
  key: string;
  field: GroupField;
  runs: RunSummary[];
  children: GroupNode[];
  expanded: boolean;
  selected: boolean;
}

interface MetricStats {
  count: number;
  avgVisibility: number;
  avgAcquisition: number;
  avgEfficiency: number;
  avgExplanation: number;
  successRate: number;
}

function computeStats(runs: RunSummary[]): MetricStats {
  const validRuns = runs.filter(r => r.visibility_score !== undefined);
  const totalRuns = runs.length;
  
  if (validRuns.length === 0) {
    return {
      count: totalRuns,
      avgVisibility: 0,
      avgAcquisition: 0,
      avgEfficiency: 0,
      avgExplanation: 0,
      successRate: totalRuns > 0 ? (runs.filter(r => r.outcome === "success").length / totalRuns) : 0,
    };
  }
  
  const sum = validRuns.reduce(
    (acc, r) => ({
      visibility: acc.visibility + (r.visibility_score ?? 0),
      acquisition: acc.acquisition + (r.acquisition_score ?? 0),
      efficiency: acc.efficiency + (r.efficiency_score ?? 0),
      explanation: acc.explanation + (r.explanation_score ?? 0),
    }),
    { visibility: 0, acquisition: 0, efficiency: 0, explanation: 0 }
  );

  const successCount = runs.filter(r => r.outcome === "success").length;

  return {
    count: totalRuns,
    avgVisibility: sum.visibility / validRuns.length,
    avgAcquisition: sum.acquisition / validRuns.length,
    avgEfficiency: sum.efficiency / validRuns.length,
    avgExplanation: sum.explanation / validRuns.length,
    successRate: successCount / totalRuns,
  };
}

function formatPct(value: number): string {
  return (value * 100).toFixed(1) + "%";
}

function getFieldValue(run: RunSummary, field: GroupField): string {
  const val = run[field];
  return val !== undefined && val !== null ? String(val) : "unknown";
}

export function Leaderboard({ onSelectRun, runs }: LeaderboardProps): ReactElement {
  const [groupBy, setGroupBy] = useState<GroupField[]>(["harness_version", "model_slug"]);
  const [selectedGroups, setSelectedGroups] = useState<Set<string>>(new Set());
  const [expandedNodes, setExpandedNodes] = useState<Set<string>>(new Set());
  const [filters, setFilters] = useState<Record<string, string>>({});
  const [sortBy, setSortBy] = useState<keyof MetricStats>("avgVisibility");
  const [compareMode, setCompareMode] = useState(false);
  const allRuns = runs || [];

  const availableFields: { value: GroupField; label: string }[] = [
    { value: "harness_version", label: "Harness Version" },
    { value: "model_slug", label: "Model" },
    { value: "strategy_id", label: "Strategy" },
    { value: "provider", label: "Provider" },
    { value: "task_id", label: "Task" },
  ];

  const uniqueValues = useMemo(() => {
    const values: Record<string, Set<string>> = {};
    availableFields.forEach(f => {
      values[f.value] = new Set(allRuns.map(r => getFieldValue(r, f.value)).filter(v => v && v !== "unknown"));
    });
    return values;
  }, [allRuns]);

  const filteredRuns = useMemo(() => {
    return allRuns.filter(run => {
      for (const [field, value] of Object.entries(filters)) {
        if (value && getFieldValue(run, field as GroupField).indexOf(value) === -1) {
          return false;
        }
      }
      return true;
    });
  }, [allRuns, filters]);

  const tree = useMemo(() => {
    function buildTree(inputRuns: RunSummary[], depth: number): GroupNode[] {
      const currentField = groupBy[depth];
      if (!currentField || depth >= groupBy.length || inputRuns.length === 0) {
        return [];
      }

      const field = currentField;
      const groups = new Map<string, RunSummary[]>();
      
      inputRuns.forEach(run => {
        const key = getFieldValue(run, field);
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(run);
      });

      return Array.from(groups.entries()).map(([key, groupRuns]) => {
        const childGroups = buildTree(groupRuns, depth + 1);
        const shortKey = key.split(" / ").pop() ?? key;
        return {
          key,
          field,
          runs: groupRuns,
          children: childGroups,
          expanded: expandedNodes.has(`${depth}-${shortKey}`),
          selected: selectedGroups.has(shortKey),
        };
      }).sort((a, b) => {
        const statsA = computeStats(a.runs);
        const statsB = computeStats(b.runs);
        const valA = statsA[sortBy] ?? 0;
        const valB = statsB[sortBy] ?? 0;
        return valB - valA;
      });
    }

    return buildTree(filteredRuns, 0);
  }, [filteredRuns, groupBy, expandedNodes, selectedGroups, sortBy]);

  const allGroupsFlat = useMemo(() => {
    function collectGroups(nodes: GroupNode[], prefix = ""): GroupNode[] {
      let result: GroupNode[] = [];
      for (const node of nodes) {
        const fullKey = prefix ? `${prefix} / ${node.key}` : node.key;
        const shortKey = node.key.split(" / ").pop() ?? node.key;
        result.push({ ...node, key: fullKey, selected: selectedGroups.has(shortKey) });
        result = result.concat(collectGroups(node.children, fullKey));
      }
      return result;
    }
    return collectGroups(tree);
  }, [tree, selectedGroups]);

  const comparisonGroups = useMemo(() => {
    if (!compareMode) return null;
    return allGroupsFlat
      .filter(g => {
        const shortKey = g.key.split(" / ").pop() ?? g.key;
        return selectedGroups.has(shortKey);
      })
      .slice(0, 4);
  }, [allGroupsFlat, selectedGroups, compareMode]);

  function toggleExpand(depth: number, key: string) {
    const nodeKey = `${depth}-${key}`;
    const newExpanded = new Set(expandedNodes);
    if (newExpanded.has(nodeKey)) {
      newExpanded.delete(nodeKey);
    } else {
      newExpanded.add(nodeKey);
    }
    setExpandedNodes(newExpanded);
  }

  function toggleSelect(key: string) {
    const newSelected = new Set(selectedGroups);
    if (newSelected.has(key)) {
      newSelected.delete(key);
    } else {
      if (newSelected.size >= 4) return;
      newSelected.add(key);
    }
    setSelectedGroups(newSelected);
  }

  function renderStatsCell(stats: MetricStats, field: keyof MetricStats): ReactElement {
    const value = stats[field];
    let colorClass = "";
    if (typeof value === "number" && field !== "count") {
      if (value >= 0.8) colorClass = "stat-excellent";
      else if (value >= 0.5) colorClass = "stat-good";
      else colorClass = "stat-poor";
    }
    return (
      <span className={colorClass}>
        {field === "count" ? value : formatPct(value)}
      </span>
    );
  }

  function renderTreeNode(node: GroupNode, depth: number): ReactElement {
    const stats = computeStats(node.runs);
    const isLeaf = node.children.length === 0;
    const shortKey = node.key.split(" / ").pop() ?? node.key;
    const nodeKey = `${depth}-${shortKey}`;
    const isExpanded = expandedNodes.has(nodeKey);
    const isSelected = selectedGroups.has(shortKey);

    return (
      <div key={node.key} className="tree-node" style={{ marginLeft: depth * 20 }}>
        <div 
          className={`tree-row ${isSelected ? "selected" : ""}`}
          onClick={() => {
            if (compareMode) {
              toggleSelect(shortKey);
            } else if (!isLeaf) {
              toggleExpand(depth, shortKey);
            }
          }}
        >
          <span className="tree-expand">
            {compareMode ? (
              <input 
                type="checkbox" 
                checked={isSelected}
                onChange={() => toggleSelect(shortKey)}
                onClick={e => e.stopPropagation()}
              />
            ) : !isLeaf ? (
              isExpanded ? "▼" : "▶"
            ) : (
              "•"
            )}
          </span>
          <span className="tree-label" style={{ fontWeight: depth === 0 ? 600 : 400 }}>
            {node.key}
          </span>
          <span className="tree-count">{stats.count} runs</span>
          <span className="tree-stat">{renderStatsCell(stats, "avgVisibility")}</span>
          <span className="tree-stat">{renderStatsCell(stats, "avgAcquisition")}</span>
          <span className="tree-stat">{renderStatsCell(stats, "avgEfficiency")}</span>
          <span className="tree-stat">{renderStatsCell(stats, "avgExplanation")}</span>
          <span className="tree-stat">{renderStatsCell(stats, "successRate")}</span>
          {!compareMode && !isLeaf && node.runs[0] && (
            <button 
              className="tree-drill"
              onClick={(e) => { 
                e.stopPropagation(); 
                const firstRun = node.runs[0];
                if (firstRun) onSelectRun(firstRun.run_id); 
              }}
            >
              View →
            </button>
          )}
        </div>
        {isExpanded && !compareMode && (
          <div className="tree-children">
            {node.children.map(child => renderTreeNode(child, depth + 1))}
            {node.runs.map(run => (
              <div key={run.run_id} className="tree-leaf" onClick={() => onSelectRun(run.run_id)}>
                <span className="tree-expand"></span>
                <span className="tree-label leaf">{run.run_id}</span>
                <span className="tree-count"></span>
                <span className="tree-stat">{formatPct(run.visibility_score ?? 0)}</span>
                <span className="tree-stat">{formatPct(run.acquisition_score ?? 0)}</span>
                <span className="tree-stat">{formatPct(run.efficiency_score ?? 0)}</span>
                <span className="tree-stat">{formatPct(run.explanation_score ?? 0)}</span>
                <span className="tree-stat">{run.outcome === "success" ? "✓" : "✗"}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }

  function renderComparisonTable(): ReactElement {
    if (!comparisonGroups || comparisonGroups.length === 0) return <></>;
    
    return (
      <div className="comparison-table">
        <table>
          <thead>
            <tr>
              <th>Metric</th>
              {comparisonGroups.map(g => (
                <th key={g.key}>{g.key}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Run Count</td>
              {comparisonGroups.map(g => <td key={g.key}>{computeStats(g.runs).count}</td>)}
            </tr>
            <tr>
              <td>Visibility</td>
              {comparisonGroups.map(g => <td key={g.key}>{renderStatsCell(computeStats(g.runs), "avgVisibility")}</td>)}
            </tr>
            <tr>
              <td>Acquisition</td>
              {comparisonGroups.map(g => <td key={g.key}>{renderStatsCell(computeStats(g.runs), "avgAcquisition")}</td>)}
            </tr>
            <tr>
              <td>Efficiency</td>
              {comparisonGroups.map(g => <td key={g.key}>{renderStatsCell(computeStats(g.runs), "avgEfficiency")}</td>)}
            </tr>
            <tr>
              <td>Explanation</td>
              {comparisonGroups.map(g => <td key={g.key}>{renderStatsCell(computeStats(g.runs), "avgExplanation")}</td>)}
            </tr>
            <tr>
              <td>Success Rate</td>
              {comparisonGroups.map(g => <td key={g.key}>{renderStatsCell(computeStats(g.runs), "successRate")}</td>)}
            </tr>
          </tbody>
        </table>
        <div style={{ marginTop: "1rem", display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
          {comparisonGroups.map(g => (
            <div key={g.key} style={{ flex: "1 1 200px", minWidth: "200px" }}>
              <div style={{ fontWeight: 600, marginBottom: "0.5rem" }}>{g.key}</div>
              <div style={{ fontSize: "0.75rem", color: "var(--color-text-muted)" }}>
                {g.runs.slice(0, 5).map(r => (
                  <div key={r.run_id} 
                    style={{ cursor: "pointer", padding: "0.25rem 0", color: "var(--color-accent-cyan)" }}
                    onClick={() => onSelectRun(r.run_id)}
                  >
                    {r.run_id}
                  </div>
                ))}
                {g.runs.length > 5 && <div>+{g.runs.length - 5} more</div>}
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  }

  const usedFields = new Set(groupBy);
  const unusedFields = availableFields.filter(f => !usedFields.has(f.value));

  return (
    <>
      <header className="content-header">
        <h2 className="content-title">Leaderboard</h2>
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          <button 
            className={`modal-btn ${compareMode ? "active" : ""}`}
            onClick={() => { setCompareMode(!compareMode); setSelectedGroups(new Set()); }}
          >
            {compareMode ? "✓ Compare Mode" : "⚖ Compare"}
          </button>
          {compareMode && selectedGroups.size > 0 && (
            <span style={{ fontSize: "0.8rem", color: "var(--color-text-muted)" }}>
              {selectedGroups.size} selected (max 4)
            </span>
          )}
        </div>
      </header>

      <div className="filter-bar" style={{ flexWrap: "wrap", gap: "0.5rem" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <span style={{ fontSize: "0.75rem", color: "var(--color-text-muted)", textTransform: "uppercase" }}>Group by:</span>
          <select
            className="filter-select"
            value={groupBy[0] || ""}
            onChange={(e) => setGroupBy([e.target.value as GroupField, ...groupBy.slice(1)])}
          >
            {availableFields.map(f => (
              <option key={f.value} value={f.value}>{f.label}</option>
            ))}
          </select>
          <button 
            className="modal-btn" 
            onClick={() => {
              const firstUnused = unusedFields[0];
              if (groupBy.length < 3 && firstUnused) {
                setGroupBy([...groupBy, firstUnused.value]);
              }
            }}
            disabled={groupBy.length >= 3 || unusedFields.length === 0}
          >
            + Add Level
          </button>
          {groupBy.slice(1).map((field, idx) => {
            const currentUsed = new Set(groupBy.slice(0, idx + 1));
            const availableForThis = availableFields.filter(f => currentUsed.has(f.value) ? f.value === field : true);
            return (
              <span key={idx} style={{ display: "flex", alignItems: "center", gap: "0.25rem" }}>
                <span style={{ color: "var(--color-text-muted)" }}>→</span>
                <select
                  className="filter-select"
                  value={field}
                  onChange={(e) => {
                    const newGroupBy = [...groupBy];
                    newGroupBy[idx + 1] = e.target.value as GroupField;
                    setGroupBy(newGroupBy);
                  }}
                >
                  {availableForThis.map(f => (
                    <option key={f.value} value={f.value}>{f.label}</option>
                  ))}
                </select>
                <button 
                  className="modal-btn" 
                  style={{ padding: "0.25rem 0.5rem", fontSize: "0.7rem" }}
                  onClick={() => setGroupBy(groupBy.filter((_, i) => i !== idx + 1))}
                >
                  ×
                </button>
              </span>
            );
          })}
        </div>

        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          <span style={{ fontSize: "0.75rem", color: "var(--color-text-muted)", textTransform: "uppercase" }}>Sort:</span>
          <select
            className="filter-select"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as keyof MetricStats)}
          >
            <option value="avgVisibility">Visibility</option>
            <option value="avgAcquisition">Acquisition</option>
            <option value="avgEfficiency">Efficiency</option>
            <option value="avgExplanation">Explanation</option>
            <option value="successRate">Success Rate</option>
            <option value="count">Run Count</option>
          </select>
        </div>

        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
          {availableFields.slice(0, 3).map(field => (
            <select
              key={field.value}
              className="filter-select"
              value={filters[field.value] || ""}
              onChange={(e) => setFilters(f => ({ ...f, [field.value]: e.target.value }))}
            >
              <option value="">All {field.label}s</option>
              {Array.from(uniqueValues[field.value] || []).map(v => (
                <option key={v} value={v}>{v}</option>
              ))}
            </select>
          ))}
        </div>
      </div>

      <div className="content-body">
        {compareMode ? (
          comparisonGroups && comparisonGroups.length > 0 ? (
            renderComparisonTable()
          ) : (
            <div className="empty-state">
              <div className="empty-icon">⚖️</div>
              <div className="empty-title">Select Groups to Compare</div>
              <div className="empty-description">Check the boxes next to groups you want to compare (max 4)</div>
            </div>
          )
        ) : (
          <div className="leaderboard-tree">
            <div className="tree-header">
              <span style={{ width: 20 }}></span>
              <span style={{ flex: 1 }}>Group</span>
              <span style={{ width: 70 }}>Runs</span>
              <span style={{ width: 70, textAlign: "right" }}>Visibility</span>
              <span style={{ width: 70, textAlign: "right" }}>Acquis.</span>
              <span style={{ width: 70, textAlign: "right" }}>Effic.</span>
              <span style={{ width: 70, textAlign: "right" }}>Expl.</span>
              <span style={{ width: 70, textAlign: "right" }}>Success</span>
              <span style={{ width: 60 }}></span>
            </div>
            {tree.length === 0 ? (
              <div className="empty-state">
                <div className="empty-icon">📭</div>
                <div className="empty-title">No Data</div>
                <div className="empty-description">No runs match your filters</div>
              </div>
            ) : (
              tree.map(node => renderTreeNode(node, 0))
            )}
          </div>
        )}
      </div>
    </>
  );
}
