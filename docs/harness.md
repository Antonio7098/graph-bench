# Harness

## Purpose

GraphBench needs a standalone runtime harness crate.

That crate exists to develop and validate the runtime layer that sits between:

- a typed task input
- a graph-backed context selection system
- a provider-backed model client
- a typed tool engine
- a fully observable turn ledger

The harness is not an orchestration system. It is the execution core that turns explicit input plus explicit context into a bounded, inspectable agent loop.

## Why It Must Be Separate

GraphBench is trying to answer questions about:

- graph traversal quality
- working-set selection quality
- prompt and context assembly
- readiness to act

Those questions become ambiguous when they are mixed with:

- project and task registries
- workflow engines
- retries and escalation policy
- merge flows
- multi-agent coordination
- checkpoint lifecycle management

The harness crate should therefore own only the runtime behavior needed for a single bounded execution.

## Core Runtime Contract

The harness should expose a provider-agnostic Rust contract.

Core ideas:

- a typed execution input enters the harness
- the harness assembles a deterministic turn request
- the model returns one validated structured response
- the harness either records thought, executes a tool action, or completes
- every state transition is recorded
- model interaction should go through the Rust `llm` crate rather than bespoke provider glue inside GraphBench

At minimum the crate should model:

- run configuration
  - turn budget
  - timeout budget
  - token budget
  - prompt headroom
- loop state
  - `init`
  - `think`
  - `act`
  - `done`
- model request and response contracts
- tool call contracts
- turn trace artifacts
- invocation result artifacts
- structured runtime errors

## Execution Model

The runtime loop should be explicit and finite.

Recommended state machine:

1. `init`
2. `think`
3. `act`
4. `done`

The important property is not the exact names. The important property is that transitions are explicit, validated, and bounded.

The harness should fail immediately when:

- a transition is invalid
- a turn budget is exceeded
- a timeout budget is exceeded
- a token budget is exceeded
- a model response is malformed
- a tool name is unknown
- a tool payload does not validate
- required trace artifacts cannot be produced

## Prompt Assembly

Prompt assembly is one of the main things GraphBench exists to develop.

It should be deterministic and built from explicit components, not hidden mutable state.

Recommended prompt inputs:

- base runtime instructions
- response contract
- objective state
- selected turn history
- active code windows
- code-navigation items
- tool contracts
- compacted summaries

Each assembled prompt should record:

- rendered prompt hash
- rendered context hash
- context window state hash
- prompt byte count
- component byte counts
- selected item counts
- omitted item counts
- compaction events
- assembly duration

GraphBench should be able to reconstruct the exact prompt contents from the recorded artifacts.

## Runtime-Local Memory

The harness should use explicit runtime-local items rather than implicit conversation memory.

Typical item families:

- user input
- assistant text
- tool call
- tool result
- code-navigation result
- compacted summary
- active code window
- controller repair item

These items are not the durable graph state. They are prompt-visible runtime artifacts derived from that state plus recent execution.

## Tool Engine

The harness should include a typed tool engine with schema-validated I/O.

Requirements:

- tools are registered explicitly
- tools are versioned explicitly
- every tool has an input schema
- every tool has an output schema
- unknown tools are rejected
- invalid payloads are rejected
- tool-call traces include latency, outcome, and failure metadata

For GraphBench, tooling should support evidence acquisition rather than broad autonomous behavior.

Typical tool families:

- repository reads
- file listing
- graph query
- source hydration
- command execution under explicit policy
- checkpoint or completion signaling

## LLM Contract

Every LLM response is an artifact, not raw text.

The harness should require every response artifact to include:

- `prompt_version`
- `model_slug`
- `provider`

And it should validate the structured response against a schema before using it.

If validation fails, the harness stops. It does not continue on best effort.

The harness should standardize its provider interaction through the Rust `llm` crate.

That integration should still preserve GraphBench-specific metadata, validation, and tracing:

- raw provider request metadata
- raw provider response metadata
- normalized response artifact
- transport timing
- retry or backoff metadata when applicable
- token usage and cost metadata when the provider exposes it

## Agent-Facing Graph API

The harness should not invent its own graph-navigation language for agents.

The intended agent-facing graph API is the Python façade exposed by UCP:

- `ucp.query(...)`
- `graph.find(...)`
- `graph.describe(...)`
- `graph.resolve(...)`
- `graph.path(...)`
- `graph.session()`
- `session.add(...)`
- `session.walk(...)`
- `session.focus(...)`
- `session.why(...)`
- `session.export(...)`
- `session.fork()`
- `session.diff(...)`
- `session.hydrate(...)` for CodeGraph
- `ucp.run_python_query(...)`
- `ucp.PythonQueryTool(...)`

This is the surface agents should target.

That means:

- model-authored graph exploration should be expressed against the Python façade
- the harness should preserve compatibility with that façade's concepts and naming
- GraphBench should evaluate graph use in terms of that agent-facing surface, not a separate GraphBench-only DSL

The harness itself is still a Rust crate.

So the boundary should be:

- agents use the Python query surface
- the harness uses the underlying Rust graphcode APIs directly when it needs in-process graph operations
- reusable graph semantics discovered by GraphBench should be added to graphcode first, then exposed through the Python façade

There is no need to build a second Rust "agent API" that merely renames existing Rust graph methods to match Python. That would duplicate wrappers, not add capability.

## Observability

Observability is the authority for what happened.

Observability must be maximal by default.

Every run and every turn should emit enough information to answer:

- what prompt was sent
- what context was visible
- what the model returned
- what tools were called
- what changed in the visible working set
- when readiness was reached
- why the run stopped

### Run-level telemetry

Every run should preserve:

- run id
- fixture id
- task id
- strategy id
- harness version
- schema version set
- prompt version
- provider
- model slug
- graph snapshot identity
- repository commit
- execution start and end timestamps
- final outcome
- final error, if any
- aggregate token counts
- aggregate latency and duration
- aggregate tool counts
- aggregate evidence metrics
- run-level hashes for critical artifacts

### Turn-level telemetry

At minimum, every turn should preserve:

- turn index
- prior and next state
- request artifact
- request schema version
- validated response artifact
- raw response capture or blob reference
- tool-call trace list
- prompt hashes
- context hashes
- rendered prompt artifact or blob reference
- rendered context artifact or blob reference
- budget usage
- latency fields
- evidence deltas
- readiness state or readiness transition
- omission reasons
- graph/session mutations
- selection provenance
- token usage by turn
- provider request id or equivalent transport identifier
- serialization hashes for replay integrity

### Event classes

The harness should emit structured events for at least:

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

### Payload capture policy

Observability should not depend on console logs.

The harness should preserve:

- structured event records
- exact rendered prompt contents
- exact rendered context contents
- exact validated response payloads
- raw provider payloads when available
- tool input and output payloads

If payload size makes full inline storage too expensive, the system should store hash-addressed blobs and keep typed references in the event stream. The important requirement is reconstructability, not inline verbosity.

## Error Model

The harness should have a stable error taxonomy.

Recommended categories:

- configuration error
- schema validation error
- prompt assembly error
- budget error
- model transport error
- malformed model output error
- tool contract error
- tool execution error
- graph context error
- persistence error
- replay integrity error

Every failure should carry:

- stable error code
- human-readable message
- recoverability flag
- relevant identifiers

## What Is In Scope

- Rust `llm` crate integration
- provider-agnostic model client boundary
- deterministic prompt assembly
- typed turn loop
- typed tool execution
- runtime-local history management
- prompt and context hashing
- readiness signaling
- trace emission
- replay support

## What Is Out of Scope

- project registry
- task planning DAGs
- workflow orchestration
- merge automation
- governance storage
- multi-agent scheduling
- global retry policy outside the invocation

## Relationship to GraphBench

GraphBench is the proving ground for the harness semantics.

The benchmark should develop:

- graph-backed selection logic
- context projection logic
- evidence accounting
- readiness thresholds
- trace schemas

The harness crate should then consume those proven rules as concrete runtime behavior.

## Implementation Direction

The harness should be its own Rust crate with clear module boundaries.

Suggested top-level modules:

- `config`
- `contracts`
- `loop`
- `llm_client`
- `prompt`
- `context`
- `tools`
- `trace`
- `errors`
- `persistence`

The crate should remain usable without a larger orchestration system wrapped around it.
