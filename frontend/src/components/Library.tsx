import { useState, useEffect, ReactElement } from "react";
import { apiClient } from "../api/client";
import { Button } from "./Button";
import { JsonToggle } from "./JsonToggle";
import { StructuredForm } from "./StructuredForm";

interface LibraryProps {
  onBack: () => void;
  onSelectTask?: (taskId: string) => void;
}

type TabType = "strategies" | "tasks" | "fixtures" | "evidence" | "prompts";

const FORM_SCHEMAS: Record<TabType, { sections: Array<{ name: string; label: string; fields: Array<{ name: string; type: string; label?: string; placeholder?: string; required?: boolean }> }> }> = {
  strategies: {
    sections: [
      {
        name: "_top",
        label: "Basic Info",
        fields: [
          { name: "name", type: "string", label: "Name", placeholder: "e.g., graph.targeted-lexical-read.v1", required: true },
          { name: "description", type: "string", label: "Description", placeholder: "Optional description" },
        ],
      },
      {
        name: "config",
        label: "Strategy Config",
        fields: [
          { name: "strategy_id", type: "string", label: "Strategy ID", placeholder: "e.g., graph.targeted-lexical-read", required: true },
          { name: "strategy_version", type: "string", label: "Version", placeholder: "e.g., v1", required: true },
          { name: "graph_discovery", type: "string", label: "Graph Discovery", placeholder: "graph_then_targeted_lexical_read" },
          { name: "projection", type: "string", label: "Projection", placeholder: "balanced" },
          { name: "reread_policy", type: "string", label: "Reread Policy", placeholder: "allow" },
        ],
      },
    ],
  },
  tasks: {
    sections: [
      {
        name: "_top",
        label: "Basic Info",
        fields: [
          { name: "task_id", type: "string", label: "Task ID", placeholder: "e.g., prepare-edit.command-surface", required: true },
        ],
      },
      {
        name: "spec",
        label: "Task Spec",
        fields: [
          { name: "title", type: "string", label: "Title" },
          { name: "task_class", type: "string", label: "Task Class" },
          { name: "difficulty", type: "string", label: "Difficulty", placeholder: "easy" },
          { name: "statement", type: "string", label: "Statement" },
          { name: "fixture_id", type: "string", label: "Fixture ID" },
          { name: "turn_budget", type: "number", label: "Turn Budget" },
        ],
      },
      {
        name: "tools",
        label: "Allowed Tools",
        fields: [
          { name: "allowed_tools", type: "array-string", label: "Tools", placeholder: "Add tool name" },
        ],
      },
    ],
  },
  fixtures: {
    sections: [
      {
        name: "_top",
        label: "Basic Info",
        fields: [
          { name: "name", type: "string", label: "Name", placeholder: "e.g., graphbench-internal", required: true },
        ],
      },
      {
        name: "config",
        label: "Fixture Config",
        fields: [
          { name: "fixture_id", type: "string", label: "Fixture ID" },
          { name: "title", type: "string", label: "Title" },
          { name: "schema_version", type: "number", label: "Schema Version" },
        ],
      },
    ],
  },
  evidence: {
    sections: [
      {
        name: "_top",
        label: "Basic Info",
        fields: [
          { name: "task_id", type: "string", label: "Task ID", required: true },
          { name: "evidence_id", type: "string", label: "Evidence ID", required: true },
        ],
      },
      {
        name: "spec",
        label: "Evidence Spec",
        fields: [
          { name: "description", type: "string", label: "Description" },
          { name: "content", type: "string", label: "Content" },
        ],
      },
    ],
  },
  prompts: {
    sections: [
      {
        name: "_top",
        label: "Basic Info",
        fields: [
          { name: "name", type: "string", label: "Name", placeholder: "e.g., system.prompt.v1", required: true },
          { name: "description", type: "string", label: "Description" },
        ],
      },
      {
        name: "template",
        label: "Prompt Template",
        fields: [
          { name: "system", type: "string", label: "System Prompt" },
          { name: "version", type: "string", label: "Version" },
        ],
      },
    ],
  },
};

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
        const config = {
          strategy_id: newItem.strategy_id,
          strategy_version: newItem.strategy_version,
          graph_discovery: newItem.graph_discovery || "graph_then_targeted_lexical_read",
          projection: newItem.projection || "balanced",
          reread_policy: newItem.reread_policy || "allow",
        };
        await apiClient.createStrategy(
          newItem.name as string,
          config as Record<string, unknown>,
          newItem.description as string | undefined
        );
      } else if (activeTab === "tasks") {
        const spec = {
          task_id: newItem.task_id,
          title: newItem.title,
          task_class: newItem.task_class,
          difficulty: newItem.difficulty,
          statement: newItem.statement,
          fixture_id: newItem.fixture_id,
          turn_budget: newItem.turn_budget,
          allowed_tools: newItem.allowed_tools,
        };
        await apiClient.createTask(
          newItem.task_id as string,
          spec as Record<string, unknown>
        );
      } else if (activeTab === "fixtures") {
        const config = {
          fixture_id: newItem.fixture_id,
          title: newItem.title,
          schema_version: newItem.schema_version || 1,
        };
        await apiClient.createFixture(
          newItem.name as string,
          config as Record<string, unknown>,
          newItem.graph_snapshot as Record<string, unknown> | undefined
        );
      } else if (activeTab === "evidence") {
        const spec = {
          description: newItem.description,
          content: newItem.content,
        };
        await apiClient.createEvidence(
          newItem.task_id as string,
          newItem.evidence_id as string,
          spec as Record<string, unknown>
        );
      } else if (activeTab === "prompts") {
        const template = {
          system: newItem.system,
          version: newItem.version,
        };
        await apiClient.createPrompt(
          newItem.name as string,
          template as Record<string, unknown>,
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
            <JsonToggle data={itemDetails} />
          ) : (
            <div className="empty-state">Select an item to view details</div>
          )}
        </div>
      </div>

      {showAddForm && (
        <div className="modal-overlay" onClick={() => setShowAddForm(false)}>
          <div className="modal-content modal-large" onClick={e => e.stopPropagation()} style={{ overflow: "auto", maxWidth: "1000px" }}>
            <h3>Add New {activeTab.slice(0, -1)}</h3>
            {activeTab !== "evidence" && (
              <StructuredForm
                sections={FORM_SCHEMAS[activeTab].sections as never}
                value={newItem}
                onChange={setNewItem}
              />
            )}
            {activeTab === "evidence" && (
              <StructuredForm
                sections={FORM_SCHEMAS.evidence.sections as never}
                value={newItem}
                onChange={setNewItem}
              />
            )}
            <div className="modal-actions">
              <Button variant="secondary" onClick={() => { setShowAddForm(false); setNewItem({}); }}>Cancel</Button>
              <Button variant="primary" onClick={handleAddItem}>Add</Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
