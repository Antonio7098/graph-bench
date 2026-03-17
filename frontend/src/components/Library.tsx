import { useState, useEffect, ReactElement } from "react";
import { apiClient } from "../api/client";
import { Button } from "./Button";

interface LibraryProps {
  onBack: () => void;
}

type TabType = "strategies" | "tasks" | "fixtures" | "evidence" | "prompts";

export function Library({ onBack }: LibraryProps): ReactElement {
  const [activeTab, setActiveTab] = useState<TabType>("strategies");
  const [strategies, setStrategies] = useState<Array<{name: string; version: number; description?: string}>>([]);
  const [tasks, setTasks] = useState<Array<{task_id: string; version: number; spec: Record<string, unknown>}>>([]);
  const [fixtures, setFixtures] = useState<Array<{name: string; version: number; config: Record<string, unknown>}>>([]);
  const [evidence, setEvidence] = useState<Array<{task_id: string; evidence_id: string; version: number}>>([]);
  const [prompts, setPrompts] = useState<Array<{name: string; version: number; description?: string}>>([]);
  const [loading, setLoading] = useState(true);
  const [selectedItem, setSelectedItem] = useState<string | null>(null);
  const [itemDetails, setItemDetails] = useState<Record<string, unknown> | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
  const [newItem, setNewItem] = useState<Record<string, unknown>>({});

  useEffect(() => {
    loadData();
  }, [activeTab]);

  async function loadData() {
    setLoading(true);
    try {
      if (activeTab === "strategies") {
        const data = await apiClient.listStrategiesWithVersions();
        setStrategies(data);
      } else if (activeTab === "tasks") {
        const data = await apiClient.listTasksWithVersions();
        setTasks(data);
      } else if (activeTab === "fixtures") {
        const data = await apiClient.listFixturesWithVersions();
        setFixtures(data);
      } else if (activeTab === "evidence") {
        const data = await apiClient.listAllEvidence();
        setEvidence(data);
      } else if (activeTab === "prompts") {
        const data = await apiClient.listPromptsWithVersions();
        setPrompts(data);
      }
    } catch (e) {
      console.error("Failed to load data:", e);
    } finally {
      setLoading(false);
    }
  }

  async function handleSelectItem(id: string) {
    setSelectedItem(id);
    try {
      if (activeTab === "strategies") {
        const data = await apiClient.getStrategy(id);
        setItemDetails(data);
      } else if (activeTab === "tasks") {
        const data = await apiClient.getTask(id);
        setItemDetails(data);
      } else if (activeTab === "fixtures") {
        const data = await apiClient.getFixture(id);
        setItemDetails(data);
      } else if (activeTab === "prompts") {
        const data = await apiClient.getPrompt(id);
        setItemDetails(data);
      }
    } catch (e) {
      console.error("Failed to load item:", e);
    }
  }

  async function handleAddItem() {
    try {
      if (activeTab === "strategies") {
        await apiClient.createStrategy(
          newItem.name as string,
          newItem.config as Record<string, unknown>,
          newItem.description as string | undefined
        );
      } else if (activeTab === "tasks") {
        await apiClient.createTask(
          newItem.task_id as string,
          newItem.spec as Record<string, unknown>
        );
      } else if (activeTab === "fixtures") {
        await apiClient.createFixture(
          newItem.name as string,
          newItem.config as Record<string, unknown>,
          newItem.graph_snapshot as Record<string, unknown> | undefined
        );
      } else if (activeTab === "prompts") {
        await apiClient.createPrompt(
          newItem.name as string,
          newItem.template as Record<string, unknown>,
          newItem.description as string | undefined
        );
      }
      setShowAddForm(false);
      setNewItem({});
      loadData();
    } catch (e) {
      console.error("Failed to add item:", e);
      alert("Failed to add item: " + (e as Error).message);
    }
  }

  const tabs: { id: TabType; label: string; count: number }[] = [
    { id: "strategies", label: "Strategies", count: strategies.length },
    { id: "tasks", label: "Tasks", count: tasks.length },
    { id: "fixtures", label: "Fixtures", count: fixtures.length },
    { id: "evidence", label: "Evidence", count: evidence.length },
    { id: "prompts", label: "Prompts", count: prompts.length },
  ];

  return (
    <div className="library-page">
      <div className="page-header">
        <Button variant="secondary" onClick={onBack}>← Back</Button>
        <h2>Library</h2>
        <Button variant="primary" onClick={() => setShowAddForm(true)}>+ Add New</Button>
      </div>

      <div className="tabs">
        {tabs.map(tab => (
          <button
            key={tab.id}
            className={`tab ${activeTab === tab.id ? "active" : ""}`}
            onClick={() => { setActiveTab(tab.id); setSelectedItem(null); }}
          >
            {tab.label} ({tab.count})
          </button>
        ))}
      </div>

      <div className="library-content">
        <div className="library-list">
          {loading ? (
            <div className="loading">Loading...</div>
          ) : activeTab === "strategies" && strategies.length === 0 ? (
            <div className="empty-state">No strategies yet. Add one to get started.</div>
          ) : activeTab === "tasks" && tasks.length === 0 ? (
            <div className="empty-state">No tasks yet. Add one to get started.</div>
          ) : activeTab === "fixtures" && fixtures.length === 0 ? (
            <div className="empty-state">No fixtures yet. Add one to get started.</div>
          ) : activeTab === "evidence" && evidence.length === 0 ? (
            <div className="empty-state">No evidence yet. Add one to get started.</div>
          ) : activeTab === "prompts" && prompts.length === 0 ? (
            <div className="empty-state">No prompts yet. Add one to get started.</div>
          ) : (
            <>
              {activeTab === "strategies" && strategies.map(s => (
                <div
                  key={s.name}
                  className={`library-item ${selectedItem === s.name ? "selected" : ""}`}
                  onClick={() => handleSelectItem(s.name)}
                >
                  <div className="item-name">{s.name}</div>
                  <div className="item-meta">v{s.version} {s.description && `• ${s.description}`}</div>
                </div>
              ))}
              {activeTab === "tasks" && tasks.map(t => (
                <div
                  key={t.task_id}
                  className={`library-item ${selectedItem === t.task_id ? "selected" : ""}`}
                  onClick={() => handleSelectItem(t.task_id)}
                >
                  <div className="item-name">{t.task_id}</div>
                  <div className="item-meta">v{t.version}</div>
                </div>
              ))}
              {activeTab === "fixtures" && fixtures.map(f => (
                <div
                  key={f.name}
                  className={`library-item ${selectedItem === f.name ? "selected" : ""}`}
                  onClick={() => handleSelectItem(f.name)}
                >
                  <div className="item-name">{f.name}</div>
                  <div className="item-meta">v{f.version}</div>
                </div>
              ))}
              {activeTab === "evidence" && evidence.map(e => (
                <div
                  key={`${e.task_id}-${e.evidence_id}`}
                  className="library-item"
                >
                  <div className="item-name">{e.evidence_id}</div>
                  <div className="item-meta">v{e.version} (for {e.task_id})</div>
                </div>
              ))}
              {activeTab === "prompts" && prompts.map(p => (
                <div
                  key={p.name}
                  className={`library-item ${selectedItem === p.name ? "selected" : ""}`}
                  onClick={() => handleSelectItem(p.name)}
                >
                  <div className="item-name">{p.name}</div>
                  <div className="item-meta">v{p.version} {p.description && `• ${p.description}`}</div>
                </div>
              ))}
            </>
          )}
        </div>

        <div className="library-detail">
          {selectedItem && itemDetails ? (
            <pre>{JSON.stringify(itemDetails, null, 2)}</pre>
          ) : (
            <div className="empty-state">Select an item to view details</div>
          )}
        </div>
      </div>

      {showAddForm && (
        <div className="modal-overlay" onClick={() => setShowAddForm(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Add New {activeTab.slice(0, -1)}</h3>
            {activeTab === "strategies" && (
              <>
                <input placeholder="Name" value={(newItem.name as string) || ""} onChange={e => setNewItem({...newItem, name: e.target.value})} />
                <textarea placeholder="Config (JSON)" value={newItem.config ? JSON.stringify(newItem.config) : ""} onChange={e => { try { setNewItem({...newItem, config: JSON.parse(e.target.value)}); } catch {} }} />
                <input placeholder="Description (optional)" value={(newItem.description as string) || ""} onChange={e => setNewItem({...newItem, description: e.target.value})} />
              </>
            )}
            {activeTab === "tasks" && (
              <>
                <input placeholder="Task ID" value={(newItem.task_id as string) || ""} onChange={e => setNewItem({...newItem, task_id: e.target.value})} />
                <textarea placeholder="Spec (JSON)" value={newItem.spec ? JSON.stringify(newItem.spec) : ""} onChange={e => { try { setNewItem({...newItem, spec: JSON.parse(e.target.value)}); } catch {} }} />
              </>
            )}
            {activeTab === "fixtures" && (
              <>
                <input placeholder="Name" value={(newItem.name as string) || ""} onChange={e => setNewItem({...newItem, name: e.target.value})} />
                <textarea placeholder="Config (JSON)" value={newItem.config ? JSON.stringify(newItem.config) : ""} onChange={e => { try { setNewItem({...newItem, config: JSON.parse(e.target.value)}); } catch {} }} />
              </>
            )}
            {activeTab === "prompts" && (
              <>
                <input placeholder="Name" value={(newItem.name as string) || ""} onChange={e => setNewItem({...newItem, name: e.target.value})} />
                <textarea placeholder="Template (JSON)" value={newItem.template ? JSON.stringify(newItem.template) : ""} onChange={e => { try { setNewItem({...newItem, template: JSON.parse(e.target.value)}); } catch {} }} />
                <input placeholder="Description (optional)" value={(newItem.description as string) || ""} onChange={e => setNewItem({...newItem, description: e.target.value})} />
              </>
            )}
            <div className="modal-actions">
              <Button variant="secondary" onClick={() => setShowAddForm(false)}>Cancel</Button>
              <Button variant="primary" onClick={handleAddItem}>Add</Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
