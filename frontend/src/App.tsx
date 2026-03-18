import { useState } from "react";
import type { ReactElement } from "react";
import { BrowserRouter, Routes, Route, useNavigate, useParams } from "react-router-dom";
import { RunList } from "./components/RunList";
import { RunDetail } from "./components/RunDetail";
import { TaskDetail } from "./components/TaskDetail";
import { Leaderboard } from "./components/Leaderboard";
import { Library } from "./components/Library";
import { mockRuns, mockRunDetail, createMockRunDetail } from "./api/mock";
import type { RunSummary, RunDetail as RunDetailType } from "./api/client";

type Mode = "demo" | "live";

function RunsPage({ mode, onSelectRun }: { mode: Mode; onSelectRun: (runId: string) => void }): ReactElement {
  const [refreshKey, setRefreshKey] = useState(0);
  const navigate = useNavigate();

  const handleSelectRun = (runId: string) => {
    onSelectRun(runId);
    navigate(`/runs/${runId}`);
  };

  const handleRunComplete = (runId: string) => {
    navigate(`/runs/${runId}`);
  };

  const handleRunStarted = (runId: string) => {
    navigate(`/runs/${runId}`);
  };

  return (
    <RunList 
      key={refreshKey}
      onSelectRun={handleSelectRun}
      onRunComplete={handleRunComplete}
      onRunStarted={handleRunStarted}
      runs={mode === "demo" ? mockRuns : undefined}
    />
  );
}

function RunPage({ mode }: { mode: Mode }): ReactElement {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();

  if (!runId) {
    navigate("/runs");
    return <div>Redirecting...</div>;
  }

  return (
    <RunDetail 
      runId={runId} 
      onBack={() => navigate("/runs")}
      detail={mode === "demo" ? createMockRunDetail(runId) : undefined}
    />
  );
}

function TaskPage({ mode }: { mode: Mode }): ReactElement {
  const { taskId } = useParams<{ taskId: string }>();
  const navigate = useNavigate();

  if (!taskId) {
    navigate("/library");
    return <div>Redirecting...</div>;
  }

  return (
    <TaskDetail
      taskId={taskId}
      onBack={() => navigate("/library")}
    />
  );
}

function LeaderboardPage({ mode }: { mode: Mode }): ReactElement {
  const navigate = useNavigate();
  return (
    <Leaderboard 
      onSelectRun={(runId) => navigate(`/runs/${runId}`)}
      runs={mode === "demo" ? mockRuns : undefined}
    />
  );
}

function LibraryPage(): ReactElement {
  const navigate = useNavigate();
  return (
    <Library 
      onBack={() => navigate("/runs")}
      onSelectTask={(taskId) => navigate(`/tasks/${taskId}`)}
    />
  );
}

function AppLayout({ mode, setMode }: { mode: Mode; setMode: (m: Mode) => void }): ReactElement {
  const navigate = useNavigate();
  const isDemo = mode === "demo";

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
            className="nav-item"
            onClick={() => navigate("/runs")}
          >
            <span className="nav-icon">📊</span>
            Runs
          </button>
          <button 
            className="nav-item"
            onClick={() => navigate("/leaderboard")}
          >
            <span className="nav-icon">🏆</span>
            Leaderboard
          </button>
          <button 
            className="nav-item"
            onClick={() => navigate("/library")}
          >
            <span className="nav-icon">📚</span>
            Library
          </button>
        </nav>
      </aside>
      <main className="main-content">
        <Routes>
          <Route path="/" element={<RunsPage mode={mode} onSelectRun={() => {}} />} />
          <Route path="/runs" element={<RunsPage mode={mode} onSelectRun={() => {}} />} />
          <Route path="/runs/:runId" element={<RunPage mode={mode} />} />
          <Route path="/leaderboard" element={<LeaderboardPage mode={mode} />} />
          <Route path="/library" element={<LibraryPage />} />
          <Route path="/tasks/:taskId" element={<TaskPage mode={mode} />} />
        </Routes>
      </main>
    </div>
  );
}

export function App(): ReactElement {
  const [mode, setMode] = useState<Mode>("live");

  return (
    <BrowserRouter>
      <AppLayout mode={mode} setMode={setMode} />
    </BrowserRouter>
  );
}
