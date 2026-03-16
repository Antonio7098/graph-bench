# Context, Tracing, and Observability

## Purpose

This document defines the trace fidelity GraphBench requires.

The standard is exact reconstruction, not approximate logging.

## Exact Context Reconstruction

For every turn, GraphBench must be able to reconstruct:

- the selected context objects
- their order
- their representation level
- their provenance
- omitted candidates and omission reasons
- per-section byte and token counts
- final rendered context contents
- stable hashes for rendered prompt and context

If this cannot be done, the benchmark is incomplete.

## Context Object Model

GraphBench should treat context as multi-resolution objects.

Recommended representation levels:

- `L0`
  - handle only
- `L1`
  - structural card
- `L2`
  - exact local excerpt
- `L3`
  - expanded excerpt set
- `L4`
  - full local region
- `L5`
  - full file

Each context object should track:

- stable id
- fixture and graph identity
- path or symbol identity
- kind
- provenance
- current representation level
- relevance or priority score
- downgrade horizon
- lease state
- evidence requirement matches

## Working-Set Selection

Selection should operate over graph-backed context objects, not raw files.

Useful object kinds:

- symbol
- symbol cluster
- file region
- dependency neighborhood
- test target
- validation artifact
- tool-result summary
- dirty edit locus

Selection should be observable at two levels:

- durable graph/session state
- the subset projected into the turn prompt

## Turn Ledger

The turn ledger is the core artifact of the system.

Every turn should preserve:

- task id
- fixture id
- run id
- model id
- strategy id
- turn index
- model directive or structured response
- tool request
- tool response
- graph/session state before selection
- graph/session state after selection
- selected context objects
- omitted candidates and reasons
- rendered context sections
- rendered byte and token counts
- context hash
- prompt hash
- readiness state
- evidence acquired this turn

The ledger should support:

- replay
- auditing
- score calculation
- regression comparison

## Runtime State Model

GraphBench should preserve task state in structured categories rather than only in named file lists.

Recommended categories:

- `seed_paths`
- `discovered_paths`
- `loaded_paths`
- `blocking_paths`
- `execution_paths`
- `verification_paths`
- `unresolved_questions`

This is important because evidence is often distributed across the repo.

## Observability Requirements

Observability is the source of truth.

At minimum the system should expose:

- graph queries issued
- graph query results chosen
- evidence matched by each result
- current working-set members
- representation levels
- downgrade horizons
- lease grants and denials
- readiness state and reasons
- context sections rendered
- omitted candidates and reasons
- per-turn evidence deltas

## Run-Level Telemetry

Every run should preserve:

- run id
- task id
- fixture id
- graph snapshot id
- harness version
- schema version set
- prompt version
- provider
- model slug
- strategy id
- start and end timestamps
- final outcome
- final error
- aggregate latency
- aggregate token counts
- aggregate tool counts
- aggregate evidence metrics

## Turn-Level Telemetry

Every turn should preserve:

- turn index
- state transition
- request artifact
- validated response artifact
- raw response capture or blob reference
- tool-call trace list
- prompt hashes
- context hashes
- rendered prompt artifact or blob reference
- rendered context artifact or blob reference
- budget usage
- latency fields
- omission reasons
- graph/session mutations
- selection provenance
- token usage by turn
- provider request identifier when available
- replay integrity hashes

## Structured Event Model

The system should emit structured event classes at least for:

- run started
- turn started
- prompt assembled
- model request sent
- model response received
- model response validated
- model response rejected
- tool requested
- tool started
- tool completed
- tool failed
- context mutated
- evidence matched
- readiness changed
- run completed
- run failed

## Payload Storage

Observability should not depend on terminal output.

The system should persist:

- structured event records
- exact rendered prompt contents
- exact rendered context contents
- validated response payloads
- raw provider payloads when available
- tool input and output payloads

When inline storage is too expensive, store hash-addressed blobs and reference them from structured events.

The requirement is exact recoverability, not inline verbosity.
