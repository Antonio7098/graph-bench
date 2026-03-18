import { useState, useEffect, ReactElement } from "react";
import { apiClient } from "../api/client";
import { JsonToggle } from "./JsonToggle";

interface TaskDetailProps {
  taskId: string;
  onBack: () => void;
}

export function TaskDetail({ taskId, onBack }: TaskDetailProps): ReactElement {
  const [task, setTask] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadTask() {
      try {
        const data = await apiClient.getTask(taskId);
        setTask(data as Record<string, unknown>);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to load task");
      } finally {
        setLoading(false);
      }
    }
    loadTask();
  }, [taskId]);

  if (loading) {
    return (
      <div className="page-container">
        <header className="content-header">
          <button className="nav-item" onClick={onBack}>← Back</button>
        </header>
        <div className="loading">Loading task...</div>
      </div>
    );
  }

  if (error || !task) {
    return (
      <div className="page-container">
        <header className="content-header">
          <button className="nav-item" onClick={onBack}>← Back</button>
        </header>
        <div className="empty-state">
          <div className="empty-title">Error</div>
          <div className="empty-description">{error || "Task not found"}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="page-container">
      <header className="content-header">
        <button className="nav-item" onClick={onBack}>← Back</button>
        <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
          <span className="run-id">Task: {taskId}</span>
        </div>
      </header>

      <div className="card">
        <div className="card-header">
          <h3 className="card-title">Task Details</h3>
        </div>
        <div className="card-body">
          <JsonToggle data={task} />
        </div>
      </div>
    </div>
  );
}
