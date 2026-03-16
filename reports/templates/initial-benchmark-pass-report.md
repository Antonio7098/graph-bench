# Initial Benchmark Pass Report

## Metadata

- Benchmark date:
- Operator:
- Fixture id:
- Task set:
- Model slug:
- Provider:
- Prompt version:
- Compared strategies:
- Protocol reference: [ops/benchmark-pass-protocol.md](/home/antonio/programming/Hivemind/graph-bench/ops/benchmark-pass-protocol.md)

## Executive Summary

State the benchmark outcome in 1 to 3 short paragraphs.

Must include:

- which strategies performed best and worst
- the main reasons for the differences
- the single highest-impact next change
- any major limits on the validity of the comparison

## Benchmark Conditions

### Tasks Reviewed

| Task id | Task spec | Evidence spec | Notes |
| --- | --- | --- | --- |
|  |  |  |  |

### Strategies Reviewed

| Strategy id | Config path | Comparison role | Notes |
| --- | --- | --- | --- |
|  |  |  |  |

### Run Artifacts

| Run id | Strategy | Turn ledger | Observability | Events | Score output |
| --- | --- | --- | --- | --- | --- |
|  |  |  |  |  |  |

## Deterministic Score Summary

| Run id | Visibility | Acquisition | Efficiency | Explanation | Recall | Precision | Irrelevant ratio | Turns to readiness | Rereads | Drift turns |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
|  |  |  |  |  |  |  |  |  |  |  |

## Comparison Findings

Summarize the main cross-strategy differences.

Required topics:

- evidence visibility differences
- evidence acquisition differences
- efficiency differences
- reread behavior differences
- readiness timing differences
- context-window allocation differences

For each claim, cite the relevant artifact paths.

## Task-Level Analysis

Repeat this section for the most important tasks, especially failures or high-cost wins.

### Task: `<TASK_ID>`

#### Task Truth

- Required facts:
- Acceptable proofs:
- Distractors:
- Verification targets:
- References:

#### What Happened

- Earliest turn with required evidence visible:
- Earliest turn with required evidence acquired:
- Earliest turn that justified `ready_to_edit`:
- Final outcome:
- References:

#### Failure Or Waste Analysis

- Traversal waste:
- Rereads:
- Post-readiness drift:
- Context-window waste:
- Omission errors or good prunes:
- References:

#### Strategy-Specific Notes

| Strategy | Strengths | Weaknesses | Evidence |
| --- | --- | --- | --- |
|  |  |  |  |

## Context Window Review

Summarize prompt-visible context behavior across the reviewed runs.

Required topics:

- sections that consistently contributed to required evidence
- sections that consumed budget with low evidence value
- signs of over-allocation
- signs of under-allocation
- compaction behavior and whether it helped or hurt
- whether rereads appear to be compensating for poor context retention

Reference `selection.rendered_sections`, prompt token counts, omission data, and strategy configs.

## Tooling And Observability Review

Assess whether the current tooling was sufficient for a fair and efficient benchmark pass.

Required topics:

- runner limitations
- strategy-selection limitations
- scoring workflow friction
- missing telemetry
- unclear or cumbersome artifact linkage

Call out any issue that materially reduced confidence in the comparison.

## Evidence Spec And Task Quality Review

Assess whether weak benchmark semantics distorted any findings.

Required topics:

- underspecified required facts
- weak distractors
- ambiguous readiness
- task wording leakage
- proof forms that were too strict or too soft

Separate benchmark-definition problems from agent or strategy problems.

## Top Improvements

Order by expected benchmark impact, not implementation convenience.

### 1. `<RECOMMENDATION>`

- Category:
- Problem:
- Why this matters:
- Supporting evidence:
- Expected metric impact:
- Confidence:
- Validation plan:

### 2. `<RECOMMENDATION>`

- Category:
- Problem:
- Why this matters:
- Supporting evidence:
- Expected metric impact:
- Confidence:
- Validation plan:

### 3. `<RECOMMENDATION>`

- Category:
- Problem:
- Why this matters:
- Supporting evidence:
- Expected metric impact:
- Confidence:
- Validation plan:

## Evidence Gaps And Limits

List anything the artifacts could not prove.

Examples:

- missing strategy parity
- incomplete task coverage
- provider instability
- missing token/cost metadata
- inability to isolate a suspected cause from current telemetry

## Appendices

### Appendix A: Artifact Index

List all paths used in the analysis.

### Appendix B: Key Trace Excerpts

Include only short excerpts or summarized observations tied to artifact references.

### Appendix C: Follow-Up Experiments

List the smallest next experiments that would validate or falsify the top recommendations.
