export interface RunFilter {
  fixture_id?: string;
  task_id?: string;
  strategy_id?: string;
  outcome?: string;
}

export interface RunSummary {
  run_id: string;
  fixture_id: string;
  task_id: string;
  strategy_id: string;
  provider: string;
  model_slug: string;
  harness_version?: string;
  started_at: string;
  completed_at: string;
  outcome: string;
  turn_count: number;
  visibility_score?: number;
  acquisition_score?: number;
  efficiency_score?: number;
  explanation_score?: number;
}

export interface EvidenceMatchRecord {
  run_id: string;
  turn_index: number;
  fact_id: string;
  matched_at: string;
}

export interface BlobReference {
  blob_id: string;
  run_id: string;
  turn_index: number | null;
  blob_type: string;
  media_type: string;
  path: string;
  byte_count: number;
}

export interface TurnTrace {
  run_id: string;
  turn_index: number;
  task_id: string;
  fixture_id: string;
  strategy_id: string;
  request: TurnRequest;
  response: TurnResponse;
  selection: TurnSelection;
  telemetry: TelemetryCounts;
  evidence_delta: string[];
  readiness_state: ReadinessState;
  readiness_reason: string;
  hashes: TurnHashSet;
  state_before?: string;
  state_after?: string;
  graph_session_before?: string;
  graph_session_after?: string;
  ordered_context_object_ids?: string[];
  tool_calls?: ToolCall[];
  tool_traces?: ToolCallTrace[];
}

export interface ToolCall {
  tool_name: string;
  payload: Record<string, unknown>;
  result?: string;
}

export interface ToolCallTrace {
  tool_name: string;
  input: Record<string, unknown>;
  output: string;
  duration_ms: number;
}

export interface TurnRequest {
  schema_version: number;
  prompt_version: string;
  prompt_hash: string;
  context_hash: string;
}

export interface TurnResponse {
  provider: string;
  model_slug: string;
  schema_version: number;
  validated: boolean;
}

export interface TurnSelection {
  selected_context_objects: string[];
  omitted_candidates: OmittedCandidate[];
  rendered_sections: RenderedSection[];
}

export interface OmittedCandidate {
  candidate_id: string;
  reason: string;
}

export interface RenderedSection {
  section_id: string;
  schema_version: number;
  title: string;
  content: string;
  byte_count: number;
  token_count: number;
}

export interface TelemetryCounts {
  prompt_bytes: number;
  prompt_tokens: number;
  latency_ms: number;
  tool_calls: number;
}

export type ReadinessState = "not_ready" | "evidence_visible" | "evidence_acquired" | "ready_to_edit";

export interface TurnHashSet {
  turn_hash: string;
}

export interface ScoreReport {
  run_id: string;
  task_id: string;
  schema_version: number;
  evidence_visibility_score: number;
  evidence_acquisition_score: number;
  evidence_efficiency_score: number;
  explanation_quality_score: number;
  metrics: ScoreMetrics;
}

export interface ScoreMetrics {
  required_evidence_recall: number;
  evidence_precision: number;
  irrelevant_material_ratio: number;
  turns_to_readiness: number;
  reread_count: number;
  post_readiness_drift_turns: number;
}

export interface RunManifest {
  run_id: string;
  schema_version: number;
  fixture_id: string;
  task_id: string;
  strategy_id: string;
  strategy_config: StrategyConfig;
  harness_version: string;
  schema_version_set: SchemaVersionSet;
  provider: string;
  model_slug: string;
  prompt_version: string;
  graph_snapshot_id: string;
  started_at: string;
  completed_at: string;
  outcome: string;
}

export interface StrategyConfig {
  schema_version: number;
  strategy_id: string;
  strategy_version: string;
  graph_discovery: GraphDiscoveryMode;
  projection: ProjectionMode;
  reread_policy: RereadMode;
  context_window: ContextWindowStrategyPolicy;
}

export type GraphDiscoveryMode = "broad_graph_discovery" | "graph_then_targeted_lexical_read";
export type ProjectionMode = "balanced" | "high_recall" | "minimal";
export type RereadMode = "allow" | "strict_no_reread";

export interface ContextWindowStrategyPolicy {
  compaction: ContextWindowCompactionPolicy;
  section_budgets: StrategySectionBudget[];
}

export interface ContextWindowCompactionPolicy {
  history_recent_items: number;
  summary_max_chars: number;
  emergency_summary_max_chars: number;
  deduplicate_tool_results: boolean;
}

export interface StrategySectionBudget {
  section_id: string;
  max_tokens: number;
  trim_direction: SectionTrimDirection;
}

export type SectionTrimDirection = "head" | "tail";

export interface SchemaVersionSet {
  fixture_manifest: number;
  task_spec: number;
  evidence_spec: number;
  strategy_config: number;
  context_object: number;
  context_window_section: number;
  turn_trace: number;
  score_report: number;
}

export interface RunDetail {
  manifest: RunManifest;
  turns: TurnTrace[];
  evidence_matches: EvidenceMatchRecord[];
  score_report: ScoreReport | null;
  blob_references: BlobReference[];
}

export class RunStoreClient {
  private baseUrl: string;

  constructor(baseUrl: string = "http://localhost:3001/api") {
    this.baseUrl = baseUrl;
  }

  async listRuns(filter?: RunFilter): Promise<RunSummary[]> {
    const params = new URLSearchParams();
    if (filter?.fixture_id) params.set("fixture_id", filter.fixture_id);
    if (filter?.task_id) params.set("task_id", filter.task_id);
    if (filter?.strategy_id) params.set("strategy_id", filter.strategy_id);
    if (filter?.outcome) params.set("outcome", filter.outcome);
    
    const response = await fetch(`${this.baseUrl}/runs?${params}`);
    if (!response.ok) throw new Error(`Failed to list runs: ${response.statusText}`);
    return response.json();
  }

  async getRun(runId: string): Promise<RunDetail> {
    const response = await fetch(`${this.baseUrl}/runs/${runId}`);
    if (!response.ok) throw new Error(`Failed to get run: ${response.statusText}`);
    return response.json();
  }

  async getRunManifest(runId: string): Promise<RunManifest> {
    const response = await fetch(`${this.baseUrl}/runs/${runId}/manifest`);
    if (!response.ok) throw new Error(`Failed to get manifest: ${response.statusText}`);
    return response.json();
  }

  async listTurns(runId: string): Promise<TurnTrace[]> {
    const response = await fetch(`${this.baseUrl}/runs/${runId}/turns`);
    if (!response.ok) throw new Error(`Failed to list turns: ${response.statusText}`);
    return response.json();
  }

  async getTurn(runId: string, turnIndex: number): Promise<TurnTrace> {
    const response = await fetch(`${this.baseUrl}/runs/${runId}/turns/${turnIndex}`);
    if (!response.ok) throw new Error(`Failed to get turn: ${response.statusText}`);
    return response.json();
  }

  async getEvidenceMatches(runId: string): Promise<EvidenceMatchRecord[]> {
    const response = await fetch(`${this.baseUrl}/runs/${runId}/evidence`);
    if (!response.ok) throw new Error(`Failed to get evidence: ${response.statusText}`);
    return response.json();
  }

  async getScoreReport(runId: string): Promise<ScoreReport | null> {
    const response = await fetch(`${this.baseUrl}/runs/${runId}/score`);
    if (response.status === 404) return null;
    if (!response.ok) throw new Error(`Failed to get score: ${response.statusText}`);
    return response.json();
  }

  async getBlobReference(runId: string, blobId: string): Promise<string> {
    const response = await fetch(`${this.baseUrl}/runs/${runId}/blobs/${blobId}`);
    if (!response.ok) throw new Error(`Failed to get blob: ${response.statusText}`);
    return response.text();
  }

  async runBenchmark(taskSpecPath: string, modelId?: string): Promise<{ run_id: string; success: boolean; status?: string }> {
    const response = await fetch(`${this.baseUrl}/runs/run`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ task_spec_path: taskSpecPath, model_id: modelId }),
    });
    if (!response.ok) throw new Error(`Failed to run benchmark: ${response.statusText}`);
    return response.json();
  }

  async listStrategies(): Promise<string[]> {
    const response = await fetch(`${this.baseUrl}/strategies`);
    if (!response.ok) throw new Error(`Failed to list strategies: ${response.statusText}`);
    return response.json();
  }

  async getStrategy(strategyId: string): Promise<Record<string, unknown>> {
    const response = await fetch(`${this.baseUrl}/strategies/${strategyId}`);
    if (!response.ok) throw new Error(`Failed to get strategy: ${response.statusText}`);
    return response.json();
  }

  async listTasks(): Promise<string[]> {
    const response = await fetch(`${this.baseUrl}/tasks`);
    if (!response.ok) throw new Error(`Failed to list tasks: ${response.statusText}`);
    return response.json();
  }

  async getTask(taskId: string): Promise<Record<string, unknown>> {
    const response = await fetch(`${this.baseUrl}/tasks/${taskId}`);
    if (!response.ok) throw new Error(`Failed to get task: ${response.statusText}`);
    return response.json();
  }

  async listFixtures(): Promise<string[]> {
    const response = await fetch(`${this.baseUrl}/fixtures`);
    if (!response.ok) throw new Error(`Failed to list fixtures: ${response.statusText}`);
    return response.json();
  }
}

export const apiClient = new RunStoreClient();
