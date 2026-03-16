# Artifacts and Schemas

## Purpose

GraphBench should define stable artifact schemas before serious benchmark data is collected.

Without schema discipline, traces drift and comparisons become expensive.

## Schema Principles

Every persisted artifact should be:

- typed
- versioned
- deterministic to serialize
- explicit about optional fields
- explicit about identity fields

All cross-boundary artifacts should be suitable for Rust type generation and TypeScript client generation.

## Required Artifact Families

The first version should define stable schemas for:

- fixture manifest
- task spec
- evidence spec
- strategy config
- context object
- turn trace
- score report
- run manifest

## Fixture Manifest

Recommended fields:

```yaml
fixture_id: string
schema_version: integer
repository:
  source: string
  commit_sha: string
  mirror_policy: string
graph:
  snapshot_id: string
  snapshot_format: string
  snapshot_ref: string
languages:
  - string
metadata:
  title: string
  notes: string
```

## Task Spec

Recommended fields:

```yaml
task_id: string
schema_version: integer
fixture_id: string
title: string
statement: string
task_class: locate|explain|prepare_to_edit
difficulty: string
allowed_tools:
  - string
turn_budget: integer
evidence_spec_ref: string
seed_paths:
  - string
seed_selectors:
  - string
verification_targets:
  - string
```

## Evidence Spec

Recommended fields:

```yaml
evidence_spec_id: string
schema_version: integer
required_facts:
  - fact_id: string
    description: string
    acceptable_proofs:
      - kind: string
        value: string
supporting_facts:
  - fact_id: string
    description: string
distractor_facts:
  - fact_id: string
    description: string
verification_targets:
  - kind: string
    value: string
```

## Context Object

Recommended fields:

```yaml
context_object_id: string
schema_version: integer
graph_snapshot_id: string
kind: string
identity:
  logical_key: string
  path: string
  symbol: string
representation_level: string
provenance:
  source_kind: string
  anchor_id: string
relevance_score: integer
lease_state: string
evidence_matches:
  - fact_id: string
hashes:
  object_hash: string
```

## Strategy Config

Recommended fields:

```yaml
schema_version: integer
strategy_id: <family>.<variant>
strategy_version: v<major>[.<minor>[.<patch>]]
graph_discovery: broad_graph_discovery|graph_then_targeted_lexical_read
projection: balanced|high_recall|minimal
reread_policy: allow|strict_no_reread
context_window:
  compaction:
    history_recent_items: integer
    summary_max_chars: integer
    emergency_summary_max_chars: integer
    deduplicate_tool_results: boolean
  section_budgets:
    - section_id: string
      max_tokens: integer
      trim_direction: head|tail
```

The strategy id format is intentionally stable and low-cardinality:

- `strategy_id` identifies the family and named variant
- `strategy_version` identifies the versioned behavior of that variant

For example:

- `graph.broad-discovery@v1`
- `graph.targeted-lexical-read@v1`
- `projection.high-recall@v1`

## Turn Trace

Recommended fields:

```yaml
run_id: string
turn_index: integer
task_id: string
fixture_id: string
strategy_id: string
request:
  schema_version: integer
  prompt_version: string
  prompt_hash: string
  context_hash: string
response:
  provider: string
  model_slug: string
  schema_version: integer
  validated: boolean
selection:
  selected_context_objects:
    - string
  omitted_candidates:
    - candidate_id: string
      reason: string
telemetry:
  prompt_bytes: integer
  prompt_tokens: integer
  latency_ms: integer
  tool_calls: integer
evidence_delta:
  acquired_fact_ids:
    - string
readiness:
  state: string
  reason: string
hashes:
  turn_hash: string
```

## Score Report

Recommended fields:

```yaml
run_id: string
task_id: string
schema_version: integer
scores:
  evidence_visibility_score: number
  evidence_acquisition_score: number
  evidence_efficiency_score: number
  explanation_quality_score: number
metrics:
  required_evidence_recall: number
  evidence_precision: number
  irrelevant_material_ratio: number
  turns_to_readiness: integer
  reread_count: integer
  post_readiness_drift_turns: integer
```

## Run Manifest

Recommended fields:

```yaml
run_id: string
schema_version: integer
fixture_id: string
task_id: string
strategy_id: string
strategy_config:
  schema_version: integer
  strategy_id: string
  strategy_version: string
harness_version: string
schema_version_set:
  fixture_manifest: integer
  task_spec: integer
  evidence_spec: integer
  strategy_config: integer
  turn_trace: integer
  score_report: integer
provider: string
model_slug: string
prompt_version: string
graph_snapshot_id: string
started_at: string
completed_at: string
outcome: string
```

## Versioning Rules

Each artifact family should evolve independently.

That means:

- each schema gets its own version
- run metadata records the version set used
- breaking changes require explicit migration or compatibility handling

## Validation Rules

Artifacts should be validated:

- at creation time
- before persistence
- before replay
- before scoring
- before UI projection

Validation failures should be hard failures.
