# Scoring and Evaluation

## Purpose

GraphBench should score evidence gathering quality and efficiency, not only final answers.

## Three Evaluation Layers

The benchmark should distinguish three different states.

### 1. Evidence Visible

The context trace shows that required evidence was visible to the model.

Examples:

- a required symbol card was rendered
- a required excerpt was rendered
- a relevant graph result was included in prompt-visible context

This answers:

- could the model have seen it?

### 2. Evidence Acquired

The agent actually retrieved the required fact through attributable traversal or reading behavior.

Examples:

- it issued the graph query that surfaced the node
- it hydrated the exact excerpt that proves the fact
- it assembled the needed pair of supporting facts across turns

This answers:

- did the agent gather the evidence?

### 3. Evidence Understood or Applied

The agent used the acquired evidence correctly in its explanation or readiness judgment.

Examples:

- it named the correct execution frontier
- it explained the dependency chain correctly
- it identified the true verification target

This answers:

- did the agent synthesize the evidence correctly?

## Recommended Scoring Split

GraphBench should compute at least:

- `evidence_visibility_score`
- `evidence_acquisition_score`
- `evidence_efficiency_score`
- `explanation_quality_score`

The first three should be the primary benchmark signals.

The last should be secondary.

## Core Metrics

Recommended metrics:

- `required_evidence_recall`
- `evidence_precision`
- `irrelevant_material_ratio`
- `turns_to_readiness`
- `graph_queries_per_required_fact`
- `reads_per_required_fact`
- `reread_count`
- `redundant_neighbor_expansion_count`
- `post_readiness_drift_turns`
- `context_window_waste_ratio`

Early iterations should prioritize:

- required evidence recall
- irrelevant material ratio
- turns to readiness
- reread count
- post-readiness drift turns

## LLM-as-Judge

Judge models can be useful, but they must not be the source of truth for core scoring.

If the benchmark only asks a judge to read the final answer, it becomes too soft.

It can:

- reward lucky summaries
- miss whether evidence was actually gathered
- hide inefficient traversal
- fail to distinguish supported claims from hallucinated ones

## Deterministic Primary Evaluation

Primary evaluation should be deterministic or oracle-based wherever possible.

Use:

- hand-authored required evidence lists
- acceptable proof forms
- exact turn traces
- exact context reconstruction
- deterministic matching

This layer should decide:

- whether required evidence was gathered
- whether it was ever visible
- whether irrelevant material was excessive
- how efficient the traversal was

Current implementation notes:

- deterministic scoring consumes a recorded turn ledger plus an `EvidenceSpec`
- visibility is matched against prompt-visible rendered sections
- acquisition is matched against recorded tool traces
- score reports are emitted without requiring any judge model
- explanation quality remains a pluggable secondary scorer layered on top

## Secondary Judge-Assisted Evaluation

Judge-assisted scoring should only be used for:

- explanation quality
- readiness correctness
- unsupported claims

It should sit on top of deterministic evidence accounting, not replace it.

The implementation should expose this as an optional interface so judge-assisted
scores can be attached later without changing the deterministic primary report.

## Artifact Split for Evaluation

Each benchmark run should produce three related artifacts:

### Evidence Oracle

Hand-authored task truth:

- required facts
- acceptable proofs
- distractors
- optional supporting facts

### Evidence Trace

Machine-recorded run truth:

- graph results surfaced
- lexical reads performed
- context objects rendered
- evidence matched per turn

### Outcome Judgment

Evaluation of the model's synthesis:

- explanation quality
- readiness correctness
- unsupported claims

## Readiness

Readiness should be task-class-specific.

Examples:

- `locate`
  - enough evidence to name the right files, symbols, or tests
- `explain`
  - enough evidence to state the real causal story without unsupported leaps
- `prepare-to-edit`
  - enough evidence to identify the true edit loci and likely verification targets

Readiness is a benchmark concept and should be recorded explicitly in traces.

## Cost Normalization

The benchmark should preserve raw cost signals first and normalize later.

Examples:

- prompt bytes
- prompt tokens
- turns
- graph queries
- hydrations
- rereads

This avoids hiding meaningful behavior behind premature normalization.
