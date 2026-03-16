import { useState, useEffect } from "react";
import type { ReactElement } from "react";
import { apiClient, type RunSummary, type ScoreReport } from "../api/client";
import { mockRuns, createMockRunDetail } from "../api/mock";

interface StrategyResult {
  strategy_id: string;
  runs: RunSummary[];
  scores: ScoreReport[];
  avg_visibility: number;
  avg_acquisition: number;
  avg_efficiency: number;
}

interface StrategyComparisonProps {
  onBack: () => void;
  runs?: RunSummary[];
}

export function StrategyComparison({ onBack, runs: propRuns }: StrategyComparisonProps): ReactElement {
  const [results, setResults] = useState<StrategyResult[]>([]);
  const [loading, setLoading] = useState(!propRuns);
  const [error, setError] = useState<string | null>(null);
  const isDemo = !!propRuns;

  useEffect(() => {
    if (!isDemo) {
      loadComparisonData();
    } else {
      loadDemoData();
    }
  }, [isDemo, propRuns]);

  async function loadComparisonData() {
    setLoading(true);
    setError(null);
    try {
      const runs = await apiClient.listRuns();
      const comparisonResults = await processRuns(runs);
      setResults(comparisonResults);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load comparison data");
    } finally {
      setLoading(false);
    }
  }

  async function loadDemoData() {
    setLoading(true);
    try {
      const strategies = ["graph.broad-discovery", "graph.targeted-lexical-read", "baseline.broad-discovery"];
      const demoResults: StrategyResult[] = [];
      
      for (const strategy_id of strategies) {
        const detail = createMockRunDetail(strategy_id);
        if (detail.score_report) {
          demoResults.push({
            strategy_id,
            runs: propRuns?.filter(r => r.strategy_id === strategy_id) || [],
            scores: [detail.score_report],
            avg_visibility: detail.score_report.evidence_visibility_score,
            avg_acquisition: detail.score_report.evidence_acquisition_score,
            avg_efficiency: detail.score_report.evidence_efficiency_score,
          });
        }
      }
      
      demoResults.sort((a, b) => b.avg_acquisition - a.avg_acquisition);
      setResults(demoResults);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load demo data");
    } finally {
      setLoading(false);
    }
  }

  async function processRuns(runs: RunSummary[]): Promise<StrategyResult[]> {
    const strategyMap = new Map<string, { runs: RunSummary[], scores: ScoreReport[] }>();
    
    for (const run of runs) {
      if (!strategyMap.has(run.strategy_id)) {
        strategyMap.set(run.strategy_id, { runs: [], scores: [] });
      }
      const entry = strategyMap.get(run.strategy_id)!;
      entry.runs.push(run);
      
      try {
        const score = await apiClient.getScoreReport(run.run_id);
        if (score) {
          entry.scores.push(score);
        }
      } catch {
        // Skip runs without scores
      }
    }
    
    const comparisonResults: StrategyResult[] = [];
    
    for (const [strategy_id, data] of strategyMap) {
      const scores = data.scores;
      if (scores.length === 0) continue;
      
      const avg_visibility = scores.reduce((sum, s) => sum + s.evidence_visibility_score, 0) / scores.length;
      const avg_acquisition = scores.reduce((sum, s) => sum + s.evidence_acquisition_score, 0) / scores.length;
      const avg_efficiency = scores.reduce((sum, s) => sum + s.evidence_efficiency_score, 0) / scores.length;
      
      comparisonResults.push({
        strategy_id,
        runs: data.runs,
        scores,
        avg_visibility,
        avg_acquisition,
        avg_efficiency,
      });
    }
    
    return comparisonResults.sort((a, b) => b.avg_acquisition - a.avg_acquisition);
  }

  function getScoreClass(score: number): string {
    if (score >= 0.8) return "excellent";
    if (score >= 0.5) return "good";
    return "poor";
  }

  return (
    <>
      <header className="content-header">
        <button className="nav-item" onClick={onBack}>← Back</button>
        <h2 className="content-title">Strategy Comparison</h2>
      </header>

      <div className="content-body">
        {loading && (
          <div className="loading">
            <div className="spinner" />
          </div>
        )}

        {error && (
          <div className="empty-state">
            <div className="empty-icon">⚠️</div>
            <div className="empty-title">Error</div>
            <div className="empty-description">{error}</div>
          </div>
        )}

        {!loading && !error && results.length === 0 && (
          <div className="empty-state">
            <div className="empty-icon">📊</div>
            <div className="empty-title">No Comparison Data</div>
            <div className="empty-description">
              Run benchmarks with multiple strategies to enable comparison.
            </div>
          </div>
        )}

        {!loading && !error && results.length > 0 && (
          <div className="comparison-grid fade-in">
            {results.map((result) => (
              <div key={result.strategy_id} className="comparison-card">
                <div className="comparison-header">
                  <div className="comparison-strategy">{result.strategy_id}</div>
                  <div style={{ fontSize: "0.8rem", color: "var(--color-text-muted)", marginTop: "0.25rem" }}>
                    {result.scores.length} runs evaluated
                  </div>
                </div>
                <div className="comparison-metrics">
                  <div className="metric-row">
                    <span className="metric-label">Avg. Visibility</span>
                    <span className={`metric-value ${getScoreClass(result.avg_visibility)}`}>
                      {(result.avg_visibility * 100).toFixed(1)}%
                    </span>
                  </div>
                  <div className="metric-row">
                    <span className="metric-label">Avg. Acquisition</span>
                    <span className={`metric-value ${getScoreClass(result.avg_acquisition)}`}>
                      {(result.avg_acquisition * 100).toFixed(1)}%
                    </span>
                  </div>
                  <div className="metric-row">
                    <span className="metric-label">Avg. Efficiency</span>
                    <span className={`metric-value ${getScoreClass(result.avg_efficiency)}`}>
                      {(result.avg_efficiency * 100).toFixed(1)}%
                    </span>
                  </div>
                  
                  <div style={{ marginTop: "1rem", paddingTop: "1rem", borderTop: "1px solid var(--color-border-muted)" }}>
                    <div className="section-title">Aggregate Metrics</div>
                    <div style={{ marginTop: "0.75rem" }}>
                      {result.scores[0] && (
                        <>
                          <div className="metric-row">
                            <span className="metric-label">Evidence Recall</span>
                            <span className="metric-value">
                              {(result.scores.reduce((s, r) => s + r.metrics.required_evidence_recall, 0) / result.scores.length * 100).toFixed(1)}%
                            </span>
                          </div>
                          <div className="metric-row">
                            <span className="metric-label">Avg. Turns to Ready</span>
                            <span className="metric-value">
                              {(result.scores.reduce((s, r) => s + r.metrics.turns_to_readiness, 0) / result.scores.length).toFixed(1)}
                            </span>
                          </div>
                          <div className="metric-row">
                            <span className="metric-label">Avg. Rereads</span>
                            <span className="metric-value">
                              {(result.scores.reduce((s, r) => s + r.metrics.reread_count, 0) / result.scores.length).toFixed(1)}
                            </span>
                          </div>
                        </>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  );
}
