# GraphBench Docs

This directory expands the project brief in `README.md` into development-facing reference documents.

The intent is to keep the repo grounded locally:

- `README.md`
  - concise project brief and direction
- `constitution.yml`
  - non-negotiable engineering rules
- `docs/`
  - implementation-facing architecture and design references

## Document Map

- `architecture.md`
  - system purpose, boundaries, core invariants, and major subsystems
- `harness.md`
  - standalone runtime harness crate and runtime-facing rules
- `codegraph.md`
  - graph substrate, working-set sessions, and agent-facing graph API boundary
- `context-window.md`
  - rendered prompt-visible context shape, selection pipeline, section ordering, and omission rules
- `tasks-and-evidence.md`
  - fixture model, task classes, evidence oracles, and proof semantics
- `context-tracing-and-observability.md`
  - context object model, exact reconstruction, turn ledger, telemetry, and replay
- `scoring-and-evaluation.md`
  - scoring layers, metrics, deterministic evaluation, and judge-assisted synthesis scoring
- `artifacts-and-schemas.md`
  - draft artifact shapes, schema boundaries, and versioning expectations
- `storage-ui-and-reproducibility.md`
  - persistent run store, visualization UI, determinism, and reproducibility rules
- `development-plan.md`
  - phased implementation plan, initial fixture plan, initial task plan, and open questions

## Reading Order

Recommended order for new development work:

1. `architecture.md`
2. `harness.md`
3. `codegraph.md`
4. `context-window.md`
5. `tasks-and-evidence.md`
6. `context-tracing-and-observability.md`
7. `scoring-and-evaluation.md`
8. `artifacts-and-schemas.md`
9. `storage-ui-and-reproducibility.md`
10. `development-plan.md`

## Documentation Rules

These docs should be treated as implementation guidance, not aspirational notes.

That means:

- use strict terminology
- keep artifact names stable
- prefer schema-first descriptions for persisted structures
- record invariants explicitly
- update the relevant doc whenever benchmark semantics change
