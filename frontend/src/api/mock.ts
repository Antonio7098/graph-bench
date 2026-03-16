import type { 
  RunSummary, 
  RunDetail, 
  TurnTrace, 
  ScoreReport, 
  EvidenceMatchRecord,
  RunManifest,
  StrategyConfig,
  ToolCall
} from "./client";

const mockStrategyConfig: StrategyConfig = {
  schema_version: 1,
  strategy_id: "graph.broad-discovery",
  strategy_version: "v1",
  graph_discovery: "broad_graph_discovery",
  projection: "balanced",
  reread_policy: "allow",
  context_window: {
    compaction: {
      history_recent_items: 5,
      summary_max_chars: 2600,
      emergency_summary_max_chars: 1000,
      deduplicate_tool_results: false,
    },
    section_budgets: [
      { section_id: "base_runtime_instructions", max_tokens: 160, trim_direction: "tail" },
      { section_id: "response_contract", max_tokens: 140, trim_direction: "tail" },
      { section_id: "objective_state", max_tokens: 180, trim_direction: "tail" },
    ],
  },
};

const mockManifest: RunManifest = {
  run_id: "demo-run-001",
  schema_version: 2,
  fixture_id: "graphbench.internal",
  task_id: "prepare-edit.schema-boundaries",
  strategy_id: "graph.broad-discovery",
  strategy_config: mockStrategyConfig,
  harness_version: "0.1.0",
  schema_version_set: {
    fixture_manifest: 1,
    task_spec: 1,
    evidence_spec: 1,
    strategy_config: 1,
    context_object: 1,
    context_window_section: 1,
    turn_trace: 1,
    score_report: 1,
  },
  provider: "mock-provider",
  model_slug: "mock-model",
  prompt_version: "v1",
  graph_snapshot_id: "sha256:".padEnd(71, "a"),
  started_at: "2024-01-15T10:00:00Z",
  completed_at: "2024-01-15T10:05:00Z",
  outcome: "success",
};

const mockTurns: TurnTrace[] = [
  {
    run_id: "demo-run-001",
    turn_index: 0,
    task_id: "prepare-edit.schema-boundaries",
    fixture_id: "graphbench.internal",
    strategy_id: "graph.broad-discovery",
    request: {
      schema_version: 1,
      prompt_version: "v1",
      prompt_hash: "sha256:".padEnd(71, "a"),
      context_hash: "sha256:".padEnd(71, "b"),
    },
    response: {
      provider: "mock-provider",
      model_slug: "mock-model",
      schema_version: 1,
      validated: true,
    },
    selection: {
      selected_context_objects: ["ctx-1", "ctx-2"],
      omitted_candidates: [],
      rendered_sections: [
        {
          section_id: "objective_state",
          schema_version: 1,
          title: "Objective State",
          content: "Task: Edit the schema validation to add a new field...",
          byte_count: 100,
          token_count: 25,
        },
      ],
    },
    telemetry: {
      prompt_bytes: 1000,
      prompt_tokens: 250,
      latency_ms: 100,
      tool_calls: 2,
    },
    evidence_delta: ["fact-1"],
    readiness_state: "not_ready",
    readiness_reason: "still gathering evidence",
    hashes: {
      turn_hash: "sha256:".padEnd(71, "c"),
    },
    tool_calls: [
      {
        tool_name: "session.expand_file",
        payload: { target: { selector: "crates/graphbench-core/src" } },
        result: "Expanded 5 symbols from crates/graphbench-core/src"
      }
    ],
    graph_session_after: JSON.stringify({
      metadata: {
        schema_version: "codegraph_session.v1",
        session_id: "cgs_demo_001",
        mutation_count: 1
      },
      context: {
        selected: {
          "node-1": { block_id: "node-1", detail_level: "skeleton", pinned: false, origin: { kind: "overview" } },
          "node-2": { block_id: "node-2", detail_level: "neighborhood", pinned: false, origin: { kind: "expand" } },
          "node-3": { block_id: "node-3", detail_level: "source", pinned: true, origin: { kind: "manual" }, hydrated_source: { path: "src/main.rs", snippet: "fn main() {}" } },
        }
      },
      relations: [
        { from: "node-1", to: "node-2", kind: "contains" },
        { from: "node-2", to: "node-3", kind: "defines" },
        { from: "node-3", to: "node-1", kind: "depends_on" },
      ]
    }),
  },
  {
    run_id: "demo-run-001",
    turn_index: 1,
    task_id: "prepare-edit.schema-boundaries",
    fixture_id: "graphbench.internal",
    strategy_id: "graph.broad-discovery",
    request: {
      schema_version: 1,
      prompt_version: "v1",
      prompt_hash: "sha256:".padEnd(71, "d"),
      context_hash: "sha256:".padEnd(71, "e"),
    },
    response: {
      provider: "mock-provider",
      model_slug: "mock-model",
      schema_version: 1,
      validated: true,
    },
    selection: {
      selected_context_objects: ["ctx-1", "ctx-2", "ctx-3"],
      omitted_candidates: [
        {
          candidate_id: "crates/graphbench-core/src/artifacts.rs",
          reason: "Hydrated source excerpt superseded the shorter summary",
        },
      ],
      rendered_sections: [
        {
          section_id: "selected_history",
          schema_version: 1,
          title: "Selected History",
          content: "Tool: read_file(path='src/lib.rs')...",
          byte_count: 500,
          token_count: 125,
        },
      ],
    },
    telemetry: {
      prompt_bytes: 1500,
      prompt_tokens: 375,
      latency_ms: 150,
      tool_calls: 3,
    },
    evidence_delta: ["fact-2", "fact-3"],
    readiness_state: "evidence_acquired",
    readiness_reason: "required facts gathered",
    hashes: {
      turn_hash: "sha256:".padEnd(71, "f"),
    },
    tool_calls: [
      {
        tool_name: "graph.find",
        payload: { name_regex: "AppError", limit: 10 },
        result: "Found 3 matches for AppError in the graph"
      },
      {
        tool_name: "session.expand_file",
        payload: { target: { selector: "crates/graphbench-core/src/error.rs" } },
        result: "Expanded error.rs with 15 symbols"
      }
    ],
    graph_session_after: JSON.stringify({
      metadata: {
        schema_version: "codegraph_session.v1",
        session_id: "cgs_demo_002",
        mutation_count: 3
      },
      context: {
        selected: {
          "error.rs": { block_id: "error.rs", detail_level: "source", pinned: true, origin: { kind: "manual" }, hydrated_source: { path: "src/error.rs", snippet: "pub struct Error {}" } },
          "artifacts.rs": { block_id: "artifacts.rs", detail_level: "neighborhood", pinned: false, origin: { kind: "expand" } },
          "main.rs": { block_id: "main.rs", detail_level: "skeleton", pinned: false, origin: { kind: "overview" } },
          "lib.rs": { block_id: "lib.rs", detail_level: "source", pinned: false, origin: { kind: "hydrate" }, hydrated_source: { path: "src/lib.rs", snippet: "pub mod error;" } },
        }
      },
      relations: [
        { from: "main.rs", to: "lib.rs", kind: "imports" },
        { from: "lib.rs", to: "error.rs", kind: "imports" },
        { from: "error.rs", to: "artifacts.rs", kind: "depends_on" },
      ]
    }),
  },
  {
    run_id: "demo-run-001",
    turn_index: 2,
    task_id: "prepare-edit.schema-boundaries",
    fixture_id: "graphbench.internal",
    strategy_id: "graph.broad-discovery",
    request: {
      schema_version: 1,
      prompt_version: "v1",
      prompt_hash: "sha256:".padEnd(71, "g"),
      context_hash: "sha256:".padEnd(71, "h"),
    },
    response: {
      provider: "mock-provider",
      model_slug: "mock-model",
      schema_version: 1,
      validated: true,
    },
    selection: {
      selected_context_objects: ["ctx-1", "ctx-2", "ctx-3", "ctx-4"],
      omitted_candidates: [],
      rendered_sections: [
        {
          section_id: "active_code_windows",
          schema_version: 1,
          title: "Active Code Windows",
          content: "Edit: fn validate() { ... }",
          byte_count: 800,
          token_count: 200,
        },
      ],
    },
    telemetry: {
      prompt_bytes: 2000,
      prompt_tokens: 500,
      latency_ms: 200,
      tool_calls: 4,
    },
    evidence_delta: [],
    readiness_state: "ready_to_edit",
    readiness_reason: "all required evidence acquired",
    hashes: {
      turn_hash: "sha256:".padEnd(71, "i"),
    },
    tool_calls: [
      {
        tool_name: "session.export",
        payload: { target: { selector: "crates/graphbench-core/src" } },
        result: "Exported 200 lines of code"
      }
    ],
    graph_session_after: JSON.stringify({
      metadata: {
        schema_version: "codegraph_session.v1",
        session_id: "cgs_demo_003",
        mutation_count: 5
      },
      context: {
        selected: {
          "final.rs": { block_id: "final.rs", detail_level: "source", pinned: true, origin: { kind: "manual" }, hydrated_source: { path: "src/final.rs", snippet: "pub fn finalize() {}" } },
          "error.rs": { block_id: "error.rs", detail_level: "source", pinned: false, origin: { kind: "hydrate" } },
          "artifacts.rs": { block_id: "artifacts.rs", detail_level: "source", pinned: false, origin: { kind: "hydrate" } },
        }
      },
      relations: [
        { from: "final.rs", to: "error.rs", kind: "calls" },
        { from: "error.rs", to: "artifacts.rs", kind: "depends_on" },
      ]
    }),
  },
];

const mockScoreReport: ScoreReport = {
  run_id: "demo-run-001",
  task_id: "prepare-edit.schema-boundaries",
  schema_version: 1,
  evidence_visibility_score: 0.95,
  evidence_acquisition_score: 0.90,
  evidence_efficiency_score: 0.85,
  explanation_quality_score: 0.88,
  metrics: {
    required_evidence_recall: 1.0,
    evidence_precision: 0.95,
    irrelevant_material_ratio: 0.05,
    turns_to_readiness: 2,
    reread_count: 0,
    post_readiness_drift_turns: 0,
  },
};

const mockEvidenceMatches: EvidenceMatchRecord[] = [
  { run_id: "demo-run-001", turn_index: 0, fact_id: "fact-1", matched_at: "2024-01-15T10:01:00Z" },
  { run_id: "demo-run-001", turn_index: 1, fact_id: "fact-2", matched_at: "2024-01-15T10:02:30Z" },
  { run_id: "demo-run-001", turn_index: 1, fact_id: "fact-3", matched_at: "2024-01-15T10:02:45Z" },
];

export const mockRuns: RunSummary[] = [
  {
    run_id: "demo-run-001",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.schema-boundaries",
    strategy_id: "graph.broad-discovery",
    provider: "openai",
    model_slug: "gpt-4o",
    harness_version: "0.1.0",
    started_at: "2024-01-15T10:00:00Z",
    completed_at: "2024-01-15T10:05:00Z",
    outcome: "success",
    turn_count: 3,
    visibility_score: 0.92,
    acquisition_score: 0.85,
    efficiency_score: 0.78,
    explanation_score: 0.88,
  },
  {
    run_id: "demo-run-002",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.turn-ledger",
    strategy_id: "graph.targeted-lexical-read",
    provider: "openai",
    model_slug: "gpt-4o",
    harness_version: "0.1.0",
    started_at: "2024-01-15T11:00:00Z",
    completed_at: "2024-01-15T11:08:00Z",
    outcome: "success",
    turn_count: 5,
    visibility_score: 0.88,
    acquisition_score: 0.82,
    efficiency_score: 0.71,
    explanation_score: 0.85,
  },
  {
    run_id: "demo-run-003",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.schema-boundaries",
    strategy_id: "baseline.broad-discovery",
    provider: "openai",
    model_slug: "gpt-4o-mini",
    harness_version: "0.1.0",
    started_at: "2024-01-15T12:00:00Z",
    completed_at: "2024-01-15T12:03:00Z",
    outcome: "failure",
    turn_count: 2,
    visibility_score: 0.45,
    acquisition_score: 0.38,
    efficiency_score: 0.62,
    explanation_score: 0.41,
  },
  {
    run_id: "demo-run-004",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.schema-boundaries",
    strategy_id: "graph.broad-discovery",
    provider: "anthropic",
    model_slug: "claude-3-5-sonnet",
    harness_version: "0.2.0",
    started_at: "2024-01-16T09:00:00Z",
    completed_at: "2024-01-16T09:04:00Z",
    outcome: "success",
    turn_count: 2,
    visibility_score: 0.95,
    acquisition_score: 0.91,
    efficiency_score: 0.85,
    explanation_score: 0.93,
  },
  {
    run_id: "demo-run-005",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.turn-ledger",
    strategy_id: "graph.broad-discovery",
    provider: "anthropic",
    model_slug: "claude-3-5-sonnet",
    harness_version: "0.2.0",
    started_at: "2024-01-16T10:00:00Z",
    completed_at: "2024-01-16T10:06:00Z",
    outcome: "success",
    turn_count: 4,
    visibility_score: 0.89,
    acquisition_score: 0.84,
    efficiency_score: 0.79,
    explanation_score: 0.87,
  },
  {
    run_id: "demo-run-006",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.schema-boundaries",
    strategy_id: "graph.targeted-lexical-read",
    provider: "openai",
    model_slug: "gpt-4o",
    harness_version: "0.2.0",
    started_at: "2024-01-16T11:00:00Z",
    completed_at: "2024-01-16T11:05:00Z",
    outcome: "success",
    turn_count: 3,
    visibility_score: 0.91,
    acquisition_score: 0.86,
    efficiency_score: 0.82,
    explanation_score: 0.89,
  },
  {
    run_id: "demo-run-007",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.turn-ledger",
    strategy_id: "baseline.broad-discovery",
    provider: "openai",
    model_slug: "gpt-4o-mini",
    harness_version: "0.1.0",
    started_at: "2024-01-16T12:00:00Z",
    completed_at: "2024-01-16T12:02:00Z",
    outcome: "failure",
    turn_count: 1,
    visibility_score: 0.32,
    acquisition_score: 0.28,
    efficiency_score: 0.55,
    explanation_score: 0.35,
  },
  {
    run_id: "demo-run-008",
    fixture_id: "graphbench.internal",
    task_id: "prepare-edit.schema-boundaries",
    strategy_id: "graph.broad-discovery",
    provider: "anthropic",
    model_slug: "claude-3-opus",
    harness_version: "0.2.0",
    started_at: "2024-01-17T09:00:00Z",
    completed_at: "2024-01-17T09:03:00Z",
    outcome: "success",
    turn_count: 2,
    visibility_score: 0.97,
    acquisition_score: 0.94,
    efficiency_score: 0.88,
    explanation_score: 0.95,
  },
];

export const mockRunDetail: RunDetail = {
  manifest: mockManifest,
  turns: mockTurns,
  evidence_matches: mockEvidenceMatches,
  score_report: mockScoreReport,
  blob_references: [],
};

export function createMockRunDetail(strategyId: string): RunDetail {
  return {
    manifest: {
      ...mockManifest,
      run_id: `${strategyId}-run`,
      strategy_id: strategyId,
      strategy_config: {
        ...mockStrategyConfig,
        strategy_id: strategyId,
      },
    },
    turns: mockTurns,
    evidence_matches: mockEvidenceMatches,
    score_report: {
      ...mockScoreReport,
      evidence_visibility_score: strategyId.includes("targeted") ? 0.92 : 0.85,
      evidence_acquisition_score: strategyId.includes("targeted") ? 0.88 : 0.78,
    },
    blob_references: [],
  };
}
