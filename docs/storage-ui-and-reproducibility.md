# Storage, UI, and Reproducibility

## Persistent Run Store

GraphBench should store the full details of every run in a persistent database.

This store should include:

- run metadata
- harness version
- schema versions
- strategy versions
- full turn ledgers
- reconstructable prompt and context artifacts
- graph/session events
- tool events
- evidence matches
- score artifacts

This store should be the canonical source for:

- replay
- comparison
- regression analysis
- UI inspection

## Harness Versioning

The harness itself is a first-class artifact.

Every run should record:

- harness version
- scoring logic version
- strategy id in `<family>.<variant>` format
- strategy implementation version in `v<major>[.<minor>[.<patch>]]` format
- full strategy config payload
- schema version set

Benchmark results are only comparable when this metadata is preserved.

## Determinism and Reproducibility

GraphBench must be designed for comparison across runs.

That means:

- pinned repo commits
- durable graph snapshot identities
- canonical ordering of context objects
- explicit strategy configurations
- stable artifact serialization
- replayable traces

Provider nondeterminism may remain, but the framework itself should be deterministic.

## Visualization UI

The UI is a core development surface, not decoration.

It should support:

- run list and run comparison
- event stream playback
- per-turn prompt/context inspection
- evidence acquisition over time
- readiness transition inspection
- graph visualization with pan and zoom
- strategy comparison
- omission and pruning analysis

## UI Data Requirements

The frontend should be able to request typed projections for:

- run summary
- turn summary
- full turn detail
- context artifact detail
- evidence match timeline
- score report
- graph/session mutation history

All UI contracts should come from typed backend schemas.

## Storage Pattern

GraphBench should store large payloads in a way that preserves exact replay without bloating every query.

Recommended split:

- relational or indexed metadata for runs, turns, evidence, and scores
- blob storage for rendered prompt/context and large raw payloads
- stable hashes linking structured rows to blobs

## Recommended Directory Intent

The repo layout should eventually support:

- `fixtures/`
- `tasks/`
- `strategies/`
- `traces/`
- `reports/`
- `schemas/`
- `scripts/`
- `docs/`

The filesystem layout is not the canonical store, but it should reflect domain boundaries clearly.
