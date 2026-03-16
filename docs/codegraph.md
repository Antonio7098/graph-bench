# CodeGraph

## Purpose

GraphBench needs a local definition of the graph substrate it will use for discovery and context selection.

In this repo, "CodeGraph" means:

- a code-specific graph extracted from a pinned repository snapshot
- represented as a structured document graph
- navigated through a programmatic working-set session API
- hydrated into lexical excerpts only when needed

CodeGraph is the durable evidence substrate. Prompt context is only a projection of it.

## Two Layers

CodeGraph is easiest to understand as two cooperating layers.

### 1. Code-specific extraction layer

This layer turns a source repository into a code graph with:

- repository nodes
- directory nodes
- file nodes
- symbol nodes
- semantic and reference edges
- code-aware selectors
- source hydration support

This is the code-intelligence layer.

### 2. Generic graph runtime layer

This layer provides:

- graph storage
- graph traversal
- path finding
- stateful working-set sessions
- explainability about why nodes are selected
- JSON and SQLite persistence

This is the reusable graph-navigation layer.

GraphBench should depend on both ideas, but keep the distinction clear:

- the code-specific layer defines what the graph means
- the generic graph layer defines how a session moves through it

## What the Extracted Graph Contains

The extracted graph should model a pinned repository snapshot as a structured graph.

Typical node classes:

- repository
- directory
- file
- symbol

Typical edge classes:

- structural containment
- references
- imports
- exports
- dependency relations
- dependent relations

The current language target set inferred from the underlying extractor is:

- Rust
- Python
- TypeScript and TSX
- JavaScript and JSX

## Build Inputs and Outputs

A build should be driven by typed input, not ambient repository state.

Required inputs:

- repository path
- pinned commit hash
- extractor configuration

Useful extractor configuration fields:

- included extensions
- excluded directories
- hidden-file policy
- parse-error policy
- max file bytes
- export-edge emission policy

Useful build outputs:

- build status
- diagnostics
- canonical fingerprint
- graph statistics
- extracted document
- incremental rebuild statistics when applicable

GraphBench should store the graph snapshot identity alongside every benchmark fixture and run.

## Identity and Selectors

CodeGraph should support stable, human-usable selectors.

Useful selector forms:

- block id
- logical key such as `symbol:src/lib.rs::add`
- repository-relative path such as `src/lib.rs`
- coderef display such as `src/lib.rs:20-40`
- unique symbol name when unambiguous

Useful metadata on nodes:

- logical key
- coderef
- language
- symbol kind
- symbol name
- exported flag

These identities matter because evidence requirements need stable proof forms.

## Programmatic Surface

GraphBench should use the programmatic API, not shell out to CLI commands for core runtime behavior.

Core graph operations:

- build a graph from a repo snapshot
- resolve a selector
- describe a node
- find nodes with regex and structural filters
- compute a path between two nodes
- open a working-set session

Core session operations:

- `seed_overview`
- `focus`
- `select`
- `expand`
- `hydrate_source`
- `collapse`
- `pin`
- `prune`
- `export`
- `render_prompt`
- `why_selected`
- `fork`
- `diff`
- `apply_recommended_actions`

## Session Semantics

The session is the important part for GraphBench.

A session is a mutable working-set projection over an immutable graph snapshot.

That means:

- the graph is durable
- the session is the current investigative state
- the prompt is a rendering of part of the session

Important session behaviors:

- `seed_overview`
  - create an initial scaffold over the repo
- `focus`
  - mark the current investigative anchor
- `select`
  - add a node intentionally
- `expand`
  - walk file, dependency, or dependent frontiers with bounded depth
- `hydrate_source`
  - attach lexical source excerpts around a code target
- `collapse`
  - remove or shrink a branch of the working set
- `pin`
  - protect important nodes from pruning
- `prune`
  - reduce the visible set while keeping the session coherent
- `why_selected`
  - explain provenance for a selected node
- `fork` and `diff`
  - branch hypotheses and compare them explicitly

This is exactly the shape GraphBench needs for evidence-centric navigation.

## Why GraphBench Needs CodeGraph

GraphBench wants to benchmark evidence acquisition, not file opening.

CodeGraph gives it the right substrate:

- path and symbol identity
- dependency-aware traversal
- provenance for selected nodes
- frontier recommendations
- lexical hydration only when required
- exportable session state
- renderable prompt projections

Without this layer, context selection degenerates into ad hoc file browsing.

## Hydration Model

GraphBench should distinguish graph discovery from lexical loading.

Recommended rule:

- use graph traversal to find candidate evidence
- hydrate source only for the top-ranked candidates that need exact text

Hydration is expensive and should be observable.

Every hydration event should preserve:

- target selector
- excerpt bounds or padding
- source path
- content hash
- reason for hydration
- relation to any evidence requirement

## Provenance and Explainability

GraphBench should preserve not just what is selected, but why it is selected.

That means recording:

- origin kind
- anchor node
- relation followed
- session mutation that introduced the node
- whether the node was pinned, focused, hydrated, or pruned

This is necessary for:

- debugging traversal quality
- scoring evidence acquisition
- explaining omissions
- comparing strategy variants

## Persistence

The graph layer should support:

- portable JSON artifacts
- durable SQLite-backed stores
- session export artifacts
- stable snapshot identifiers

GraphBench should persist:

- the fixture graph snapshot
- the session state used for each turn
- the rendered export or prompt projection used at each turn

## Benchmark Usage Rules

CodeGraph usage inside GraphBench should follow these rules:

1. Build against pinned commits only.
2. Treat the graph snapshot as a versioned fixture artifact.
3. Keep graph state durable across turns.
4. Record session mutations explicitly.
5. Record omitted candidates and omission reasons.
6. Hydrate source only when needed for proof.
7. Store exact exported context used for each model turn.
8. Prefer programmatic APIs over CLI text parsing.

## Relationship to the Harness

The harness consumes prompt-ready context.

CodeGraph is how GraphBench decides what that context should be.

The boundary should stay clean:

- CodeGraph owns graph extraction, traversal, selectors, session state, and source hydration
- the harness owns prompt assembly, tool execution, model interaction, and turn traces

GraphBench sits between them and determines whether the resulting behavior is actually good.
