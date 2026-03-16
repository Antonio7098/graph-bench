# GraphBench

GraphBench is a graph-first benchmark and development environment for code navigation, evidence gathering, context-window construction, and strategy evaluation.

Its purpose is narrower than “can the agent complete the task end to end?”.

It exists to answer questions like:

- did the system discover the right evidence through graph traversal?
- did it gather that evidence efficiently?
- did it construct the right context window?
- can we reconstruct exactly what the model saw on each turn?

## Core Idea

The benchmark unit is **evidence acquired**.

GraphBench should reward:

- acquiring the right evidence
- avoiding distractors
- reaching readiness efficiently
- producing exact, replayable observability

It should not reward file-count theater or vague “the answer was somewhere in the prompt” success.

## Scope

GraphBench focuses on:

- graph-backed discovery
- working-set selection
- context-window shaping
- exact context reconstruction
- deterministic tracing and replay
- evidence-centric scoring

GraphBench does not primarily focus on:

- end-to-end code editing quality
- mergeability
- orchestration policy
- generic coding-benchmark behavior

## Key Principles

- The graph is the durable substrate.
- The context window is a projection, not the memory system.
- Observability is the source of truth.
- Reproducibility is mandatory.
- Responses, traces, and persisted artifacts are typed and versioned.

## Repo Map

- `constitution.yml`
  - engineering rules and non-negotiable constraints
- `docs/`
  - architecture and implementation references
- `ops/ROADMAP.md`
  - checklist-driven implementation tracker
- `crates/`
  - Rust workspace crates for core domain logic and the harness
- `frontend/`
  - TypeScript + React + Vite UI surface
- `schemas/`, `fixtures/`, `tasks/`, `strategies/`, `traces/`, `reports/`
  - benchmark domain directories established by the roadmap

## Commands

The initial repo-wide command surface is:

- `cargo fmt --all`
- `cargo xlint`
- `cargo xtypecheck`
- `cargo xtest`
- `just build`
- `just lint`
- `just typecheck`
- `just test`

## Docs

Start with [docs/index.md](/home/antonio/programming/Hivemind/graph-bench/docs/index.md).

Important references:

- [architecture.md](/home/antonio/programming/Hivemind/graph-bench/docs/architecture.md)
- [harness.md](/home/antonio/programming/Hivemind/graph-bench/docs/harness.md)
- [codegraph.md](/home/antonio/programming/Hivemind/graph-bench/docs/codegraph.md)
- [context-window.md](/home/antonio/programming/Hivemind/graph-bench/docs/context-window.md)
- [tasks-and-evidence.md](/home/antonio/programming/Hivemind/graph-bench/docs/tasks-and-evidence.md)
- [context-tracing-and-observability.md](/home/antonio/programming/Hivemind/graph-bench/docs/context-tracing-and-observability.md)
- [scoring-and-evaluation.md](/home/antonio/programming/Hivemind/graph-bench/docs/scoring-and-evaluation.md)
- [artifacts-and-schemas.md](/home/antonio/programming/Hivemind/graph-bench/docs/artifacts-and-schemas.md)
- [storage-ui-and-reproducibility.md](/home/antonio/programming/Hivemind/graph-bench/docs/storage-ui-and-reproducibility.md)
- [development-plan.md](/home/antonio/programming/Hivemind/graph-bench/docs/development-plan.md)

## Current Direction

The current implementation direction is:

1. define typed schemas and fixture/task artifacts
2. integrate graphcode as the graph substrate
3. build the standalone harness crate
4. implement exact turn tracing and replay
5. compare strategy variants on pinned fixtures
6. persist and visualize runs with full observability

The active execution checklist lives in [ops/ROADMAP.md](/home/antonio/programming/Hivemind/graph-bench/ops/ROADMAP.md).
