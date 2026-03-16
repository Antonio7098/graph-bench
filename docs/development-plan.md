# Development Plan

## Purpose

This document turns the README roadmap into an implementation-oriented development sequence.

## Initial Fixture Plan

Do not start with a large external fixture zoo.

Start with:

1. this repo
2. one medium external Rust or TypeScript repo
3. one larger repo later, after the benchmark semantics are stable

The priority is correctness of semantics, not breadth of fixtures.

## Initial Task Plan

Start with:

- 10 to 20 hand-authored tasks
- mostly `prepare-to-edit`

This should be enough to reveal:

- evidence recall failures
- traversal waste
- reread behavior
- post-readiness drift
- prompt/context over-allocation

## Phased Plan

### Phase 1: Spec and Trace Foundation

Build:

- fixture manifest schema
- task spec schema
- evidence spec schema
- turn ledger schema
- exact context reconstruction format

Exit criteria:

- one run can be replayed exactly from persisted artifacts

### Phase 2: Single-Repo Internal Benchmark

Build:

- one pinned internal fixture
- initial task corpus
- initial scoring pipeline
- initial reports

Exit criteria:

- at least two strategy variants can be compared on the same tasks

### Phase 3: Working-Set Selection Experiments

Build:

- graph-backed context object store
- priority or relevance ranking variants
- representation-level selection
- omission reasoning

Exit criteria:

- recall and precision tradeoffs between context strategies are measurable

### Phase 4: Readiness and Drift Measurement

Build:

- readiness definitions by task class
- post-readiness drift metrics
- reread metrics
- redundant traversal metrics

Exit criteria:

- the system can identify when the model had enough evidence and still failed to act

### Phase 5: External Fixtures

Build:

- medium external fixtures
- broader task corpus

Exit criteria:

- results generalize beyond the seed fixture repo

### Phase 6: Harness Integration

Build:

- adapter layer from proven GraphBench selection logic into the harness crate
- parity telemetry
- side-by-side runtime comparison

Exit criteria:

- integrated runtime behavior improves on GraphBench-defined metrics

## Immediate Next Steps

Recommended first implementation steps:

1. define the artifact schemas
2. define the first internal fixture at a pinned commit
3. author the first `prepare-to-edit` tasks
4. implement a turn ledger with exact rendered-context reconstruction
5. build an initial report surface for recall, waste, readiness, and rereads
6. compare at least two simple strategy variants

## Strategy Variants to Compare Early

Useful early variants:

- broad graph discovery
- graph then targeted lexical read
- high-recall context projection
- minimal context projection
- strict no-reread

Every strategy should have an explicit strategy id and config payload.

## Open Questions

The following questions should remain explicit until resolved:

- What is the best canonical evidence unit: path, symbol, logical key, excerpt, or composite?
- How should readiness differ across task classes?
- How much lexical detail counts as edit-ready?
- How should graph-derived evidence and lexical proof be merged in scoring?
- Which distractor patterns best predict real failures?
- How should cost be normalized across models with different tokenization and tool habits?

## Documentation Expectation

Each phase should update:

- the relevant schema docs
- the relevant architecture docs
- the relevant benchmark semantics docs

Development should not outrun the documentation for benchmark-critical behavior.
