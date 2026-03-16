export type MirrorPolicy = "workspace" | "local_cache_only" | "mirror_required";
export type TaskClass = "locate" | "explain" | "prepare_to_edit";
export type Difficulty = "easy" | "medium" | "hard";
export type GraphDiscoveryMode =
  | "broad_graph_discovery"
  | "graph_then_targeted_lexical_read";
export type ProjectionMode = "balanced" | "high_recall" | "minimal";
export type RereadMode = "allow" | "strict_no_reread";
export type SectionTrimDirection = "head" | "tail";
export type ProofKind =
  | "path"
  | "symbol"
  | "logical_key"
  | "excerpt"
  | "graph_path"
  | "coderef";

export interface VerificationTarget {
  kind: string;
  value: string;
}

export interface AcceptableProof {
  kind: ProofKind;
  value: string;
}

export interface EvidenceFact {
  fact_id: string;
  description: string;
  acceptable_proofs: AcceptableProof[];
}

export interface EvidenceSpec {
  evidence_spec_id: string;
  schema_version: 1;
  required_facts: EvidenceFact[];
  supporting_facts: EvidenceFact[];
  distractor_facts: EvidenceFact[];
  verification_targets: VerificationTarget[];
}

export interface StrategySectionBudget {
  section_id: string;
  max_tokens: number;
  trim_direction: SectionTrimDirection;
}

export interface ContextWindowCompactionPolicy {
  history_recent_items: number;
  summary_max_chars: number;
  emergency_summary_max_chars: number;
  deduplicate_tool_results: boolean;
}

export interface ContextWindowStrategyPolicy {
  compaction: ContextWindowCompactionPolicy;
  section_budgets: StrategySectionBudget[];
}

export interface StrategyConfig {
  schema_version: 1;
  strategy_id: string;
  strategy_version: string;
  graph_discovery: GraphDiscoveryMode;
  projection: ProjectionMode;
  reread_policy: RereadMode;
  context_window: ContextWindowStrategyPolicy;
}

export interface TaskReview {
  required_evidence_is_sufficient: true;
  distractors_are_realistic: true;
  multiple_valid_paths_considered: true;
}

export interface TaskSpec {
  task_id: string;
  schema_version: 1;
  fixture_id: string;
  title: string;
  statement: string;
  task_class: TaskClass;
  difficulty: Difficulty;
  allowed_tools: string[];
  turn_budget: number;
  evidence_spec_ref: string;
  seed_paths: string[];
  seed_selectors: string[];
  verification_targets: VerificationTarget[];
  known_distractor_regions: string[];
  expected_edit_loci: string[];
  review: TaskReview;
}

export interface RunSchemaVersionSet {
  fixture_manifest: 1;
  task_spec: 1;
  evidence_spec: 1;
  strategy_config: 1;
  context_object: 1;
  context_window_section: 1;
  turn_trace: 1;
  score_report: 1;
}

export interface RunManifest {
  run_id: string;
  schema_version: 2;
  fixture_id: string;
  task_id: string;
  strategy_id: string;
  strategy_config: StrategyConfig;
  harness_version: string;
  schema_version_set: RunSchemaVersionSet;
  provider: string;
  model_slug: string;
  prompt_version: string;
  graph_snapshot_id: string;
  started_at: string;
  completed_at: string;
  outcome: string;
}
