# Tasks and Evidence

## Purpose

This document defines how GraphBench should describe benchmark tasks and how those tasks should be scored in evidence-centric terms.

## Pinned Fixtures

Every task runs against a reproducible fixture.

A fixture should define:

- repository source
- exact commit SHA
- local cache or mirror policy
- graph snapshot identity
- language metadata
- optional fixture notes

Never benchmark against moving `HEAD`.

## Task Definition

Each task should be a typed artifact, not a paragraph in a document.

Minimum fields:

- `task_id`
- `fixture_id`
- `title`
- `statement`
- `task_class`
- `difficulty`
- `allowed_tools`
- `turn_budget`
- `evidence_spec_ref`

Optional but useful:

- `seed_paths`
- `seed_selectors`
- `known_distractor_regions`
- `expected_edit_loci`
- `verification_targets`

## Task Classes

GraphBench should start with three task classes.

### Locate

Goal:

- identify files, symbols, tests, or subgraphs relevant to a behavior

Primary success signals:

- required evidence recall
- irrelevant material ratio
- turns to sufficient localization

### Explain

Goal:

- gather enough evidence to explain a bug, behavior, dependency, or failure mode

Primary success signals:

- factual completeness
- evidence precision
- distractor avoidance

### Prepare-to-Edit

Goal:

- gather the exact evidence needed to become edit-ready, but stop before writing

Primary success signals:

- correctness of the execution frontier
- lexical readiness at the true edit loci
- low reread count
- low post-readiness drift

This should be the most important early class because it isolates whether discovery and context shaping are good enough to support action.

## EvidenceSpec

Every task should reference an `EvidenceSpec`.

Minimum fields:

- `required_facts`
- `acceptable_proofs`
- `supporting_facts`
- `distractor_facts`
- `verification_targets`

## Required Facts

A required fact is a fact the system must gather to count the task as successful.

Examples:

- a symbol is the actual implementation frontier
- a file contains the real validation target
- a dependency path explains the observed behavior

Required facts should be small, testable, and independently matchable.

## Acceptable Proofs

Each required fact should list acceptable proof forms.

Typical proof forms:

- exact path and symbol
- path plus exact excerpt
- logical key plus excerpt
- graph path between two selectors
- hydrated coderef range

Proof forms matter because multiple traversal styles may still converge on the same true evidence.

## Supporting Facts

Supporting facts are useful but not mandatory.

They should improve explanation quality or confidence without being required for correctness.

## Distractor Facts

Distractor facts define plausible false leads.

They are important because GraphBench is meant to measure navigation quality under realistic confusion, not only on clean toy paths.

Distractors should include:

- nearby files with similar names
- adjacent symbols with related payload types
- structurally similar but irrelevant tests
- import or export chains that look promising but do not establish the required fact

## Verification Targets

Tasks should preserve likely verification targets even if the benchmark class stops short of editing.

Examples:

- test file selectors
- command targets
- runtime checks
- validation artifacts

These are especially important for `prepare-to-edit`, where edit readiness depends partly on knowing how correctness would be checked.

## Multiple Valid Paths

Task design should allow multiple valid traversal paths where possible.

The benchmark should test whether the agent acquires the right evidence efficiently, not whether it follows one predetermined route.

## Quality Criteria for Task Authoring

Each task should be reviewed to ensure:

- required evidence is actually sufficient
- distractors are realistic
- acceptable proofs are explicit
- the task statement does not leak the answer
- the task can be scored from recorded artifacts

## Recommended Early Corpus

Start with:

- 10 to 20 hand-authored tasks
- mostly `prepare-to-edit`
- one seed fixture repo

This is enough to reveal real traversal and context problems while staying reviewable by hand.
