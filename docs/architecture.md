# Architecture

## Purpose

GraphBench is a graph-first benchmark and development environment for:

- evidence acquisition
- working-set selection
- context projection
- exact context reconstruction
- strategy comparison

Its benchmark unit is not file visitation and not final task completion in isolation.

Its benchmark unit is evidence acquired.

## Problem Statement

In a full execution runtime, the following concerns are often entangled:

- graph traversal
- working-set selection
- prompt assembly
- tool execution
- provider behavior
- edit commitment
- verification and retries

When a run fails, those concerns blur together.

GraphBench exists to separate the discovery and context layers from the rest.

## System Boundaries

GraphBench is responsible for:

- pinned repo fixtures
- graph-backed benchmark tasks
- evidence specifications and oracles
- graph/session-driven discovery traces
- exact prompt/context reconstruction
- evidence-centric scoring
- persistent run storage
- run inspection UI

GraphBench is not responsible for:

- full project orchestration
- merge automation
- multi-agent scheduling
- end-to-end shipping decisions

## Core Invariants

The following invariants should hold across the system:

1. Evidence is the benchmark unit.
2. Graph state is durable; prompt context is a projection.
3. Exact reconstruction is mandatory.
4. Observability artifacts are the source of truth.
5. Reproducibility beats convenience.
6. All persisted boundaries are typed and versioned.

## Major Subsystems

### 1. Fixture Layer

The fixture layer defines reproducible benchmark inputs:

- repository source
- pinned commit
- graph snapshot
- fixture metadata

### 2. Task Layer

The task layer defines benchmark work in evidence-centric terms:

- task statement
- task class
- turn budget
- allowed tools
- evidence oracle

### 3. Graph and Working-Set Layer

This layer manages durable graph-backed state and session-style working-set projection.

It should be built on CodeGraph plus the generic graph runtime, not on ad hoc file browsing.

### 4. Harness Layer

The harness is a standalone Rust crate that handles:

- turn loop
- prompt assembly
- `llm` crate integration
- tool execution
- turn-level trace emission

GraphBench develops the selection and observability rules that the harness should consume.

### 5. Evaluation Layer

This layer computes:

- evidence visibility
- evidence acquisition
- evidence efficiency
- synthesis quality

### 6. Persistence and UI Layer

This layer stores and presents:

- runs
- turn ledgers
- context artifacts
- graph/session events
- score artifacts
- visual run inspection

## Working-Set Model

The graph is the durable memory substrate.

The working set is a turn-local projection over graph-backed context objects.

That projection should be guided by:

- relevance or priority
- evidence requirements
- lexical need
- representation level
- readiness state

GraphBench should never treat prompt context as the primary memory system.

## Relationship Between Components

The intended data flow is:

1. load a pinned fixture
2. load or build the graph snapshot
3. load a task spec and evidence oracle
4. create a working-set session
5. project context into the harness
6. execute a bounded run
7. record turn artifacts and evidence deltas
8. score the run against the oracle
9. store the full run for replay and inspection

## Technology Direction

Per the constitution:

- backend implementation should be Rust
- frontend implementation should be TypeScript with React and Vite
- all artifact contracts should be strictly typed
- no subsystem should depend on informal JSON shapes or unversioned payloads

## Design Consequences

This architecture implies:

- schema design must happen early
- turn-level trace fidelity is not optional
- graph/session semantics should live in graphcode where reusable
- benchmark-only policy should stay in GraphBench
- runtime-only behavior should stay in the harness crate
