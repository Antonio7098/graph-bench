import { useState } from "react";
import type { ReactElement } from "react";
import { RunList } from "./components/RunList";
import { RunDetail } from "./components/RunDetail";
import { StrategyComparison } from "./components/StrategyComparison";
import { Leaderboard } from "./components/Leaderboard";
import { mockRuns, mockRunDetail, createMockRunDetail } from "./api/mock";
import type { RunSummary, RunDetail as RunDetailType } from "./api/client";

type View = "runs" | "detail" | "comparison" | "leaderboard";
type Mode = "demo" | "live";

export function App(): ReactElement {
  const [currentView, setCurrentView] = useState<View>("runs");
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [mode, setMode] = useState<Mode>("live");
  const isDemo = mode === "demo";

  const handleSelectRun = (runId: string) => {
    setSelectedRunId(runId);
    setCurrentView("detail");
  };

  const handleBack = () => {
    setSelectedRunId(null);
    setCurrentView("runs");
    setRefreshKey(k => k + 1);
  };

  const handleCompare = () => {
    setCurrentView("comparison");
  };

  const handleRunComplete = (runId: string) => {
    setSelectedRunId(runId);
    setCurrentView("detail");
    setRefreshKey(k => k + 1);
  };

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="sidebar-header">
          <div className="sidebar-logo">GraphBench</div>
          <button 
            onClick={() => setMode(isDemo ? "live" : "demo")}
            style={{ 
              background: "transparent", 
              border: "1px solid var(--color-border)", 
              borderRadius: "4px",
              padding: "0.25rem 0.5rem",
              fontSize: "0.6rem",
              color: mode === "demo" ? "var(--color-accent-amber)" : "var(--color-accent-green)",
              cursor: "pointer",
              marginTop: "0.25rem",
              textTransform: "uppercase",
              letterSpacing: "0.1em"
            }}
          >
            {mode === "demo" ? "Demo" : "Live"}
          </button>
        </div>
        <nav className="sidebar-nav">
          <button 
            className={`nav-item ${currentView === "runs" ? "active" : ""}`}
            onClick={() => { setCurrentView("runs"); setSelectedRunId(null); }}
          >
            <span className="nav-icon">📊</span>
            Runs
          </button>
          <button 
            className={`nav-item ${currentView === "leaderboard" ? "active" : ""}`}
            onClick={() => { setCurrentView("leaderboard"); setSelectedRunId(null); }}
          >
            <span className="nav-icon">🏆</span>
            Leaderboard
          </button>
          <button 
            className={`nav-item ${currentView === "comparison" ? "active" : ""}`}
            onClick={handleCompare}
          >
            <span className="nav-icon">⚖️</span>
            Strategy Comparison
          </button>
        </nav>
      </aside>
      <main className="main-content">
        {currentView === "runs" && (
          <RunList 
            key={refreshKey}
            onSelectRun={handleSelectRun}
            onRunComplete={handleRunComplete}
            runs={isDemo ? mockRuns : undefined}
          />
        )}
        {currentView === "leaderboard" && (
          <Leaderboard 
            onSelectRun={handleSelectRun}
            runs={isDemo ? mockRuns : undefined}
          />
        )}
        {currentView === "detail" && selectedRunId && (
          <RunDetail 
            runId={selectedRunId} 
            onBack={handleBack}
            detail={isDemo ? mockRunDetail : undefined}
          />
        )}
        {currentView === "comparison" && (
          <StrategyComparison 
            onBack={() => setCurrentView("runs")} 
            runs={isDemo ? mockRuns : undefined}
          />
        )}
      </main>
    </div>
  );
}
