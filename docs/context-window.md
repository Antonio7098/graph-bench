# Context Window

## Purpose

This document explains what the runtime-visible context window should look like, how it is constructed, and how GraphBench should reason about it.

This is a prompt-facing document, not a graph-facing document.

The graph is the durable substrate.
The context window is the rendered projection sent to the model for one turn.

## Core Principle

The context window should be:

- deliberate
- typed in origin, even if rendered as text
- budget-aware
- exactly reconstructable
- evidence-oriented

The context window is not a memory dump and not a raw transcript splice.

It is a bounded projection assembled from:

- task state
- graph-backed working-set state
- runtime-local turn artifacts
- tool contracts
- compacted summaries

## What the Model Should See

At a high level, a turn context window should contain these conceptual sections:

1. base runtime instructions
2. response contract
3. objective state
4. selected history
5. active code windows
6. code-navigation items
7. tool contracts

Not every turn will need every section, but the assembly model should stay stable.

## Recommended Render Order

Recommended order:

### 1. Base Runtime Instructions

This section defines:

- the overall runtime behavior
- hard constraints
- response formatting expectations
- high-level benchmark framing if needed

This should be stable for a prompt version.

### 2. Response Contract

This section defines the expected output schema for the next model response.

It should make clear:

- what response kinds are allowed
- what tool-call structure is allowed
- what completion looks like
- what invalid responses are

This section is critical because GraphBench requires schema-validated responses.

### 3. Objective State

This section defines the current task-facing state.

It should include:

- task statement
- task class
- relevant turn budget information
- current readiness state if applicable
- unresolved questions
- known execution or verification targets when relevant

This section tells the model what problem it is trying to solve now.

### 4. Selected History

This section contains the recent visible slice of one chronological runtime stream.

Typical item families:

- prior assistant reasoning or summaries
- prior tool calls
- prior tool results
- controller repair messages
- prior explicit state transitions
- edit event summaries

This should not be a raw append-only transcript dump.

It should be curated and compacted when necessary.

Older parts of the same stream should be summarized in place as part of the history rendering model, not treated as a separate memory system.

When edits occur, history should usually preserve structured edit metadata rather than raw code.

Examples of good history items:

- edited path
- editing tool used
- affected selector or symbol
- line range or coderef
- short edit summary
- content hash before and after

The full edited code should not be duplicated into history by default.

History should therefore support two presentation modes within the same stream:

- recent items shown at higher fidelity
- older items shown as compacted summaries

### 5. Active Code Windows

This section contains the highest-value lexical material currently visible to the model.

Typical contents:

- hydrated source excerpts
- dirty edit loci
- exact local proof excerpts
- selected file regions

This is where exact text should go when the benchmark requires lexical proof.

When an edit has just been made, the edited code should normally appear here, not in selected history.

That means the default split is:

- history carries structured edit events
- active code windows carry exact code text

Only put raw diff or full edited text into history when there is a specific reason to do so.

### 6. Code-Navigation Items

This section contains non-lexical graph-backed evidence.

Typical contents:

- symbol cards
- path summaries
- dependency neighborhoods
- file or symbol identities
- graph query results
- verification target summaries

This section should carry enough structure to guide navigation without consuming lexical budget unnecessarily.

### 7. Tool Contracts

This section defines which tools are currently available and how they should be called.

It should include:

- tool names
- version or contract identity
- input expectations
- high-level behavior notes

This should be concise but precise.

## One Stream, Progressive Compaction

The model should not be given two unrelated histories.

The intended model is:

- one sequential runtime stream
- recent items shown at higher fidelity
- older items progressively summarized

So the distinction is:

- `selected history`
  - a rendered view over the sequential stream, containing both recent high-fidelity items and older compressed items

This preserves sequential context while preventing the context window from collapsing into a full raw transcript.

Compaction should therefore preserve links back to the original stream items or stable item ids wherever possible.

## Context Window Inputs

The rendered context window should be assembled from explicit inputs, not hidden mutable state.

Required inputs include:

- prompt version
- task statement
- task class
- current readiness state
- graph/session state
- selected context objects
- selected runtime-local history items
- tool contracts
- compaction outputs
- budget configuration

Optional but useful inputs include:

- strategy id
- strategy config
- unresolved questions
- evidence facts already acquired
- verification targets

## Selection Pipeline

Recommended assembly pipeline:

1. start from durable graph/session state plus runtime-local state
2. identify mandatory items
3. identify high-priority candidate items
4. assign representation levels
5. allocate lexical budget to items that need exact text
6. allocate structural budget to graph or summary items
7. omit low-priority items with explicit reasons
8. compact stale but useful items
9. render sections in canonical order
10. hash and persist the result

## Mandatory Items

Mandatory items are items that must appear if they exist for the current turn.

Examples:

- base runtime instructions
- response contract
- current task statement
- required tool contract descriptions
- exact lexical proof needed to support the current frontier

Mandatory does not mean large. It means semantically required.

## Candidate Ranking

After mandatory items, candidate items should be ranked.

Ranking signals may include:

- evidence relevance
- current task class
- closeness to execution frontier
- verification relevance
- recency
- lease state
- representation level floor
- whether the item satisfies a currently blocking evidence requirement

The ranking policy itself should be observable and versioned.

## Representation Levels

The same underlying context object may appear at different levels.

Recommended levels:

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

The context window should prefer the lowest level that is sufficient for the current turn.

## Lexical Versus Structural Budget

GraphBench should explicitly distinguish:

- lexical budget
  - exact code and text excerpts
- structural budget
  - summaries, symbol cards, path descriptions, graph results

Lexical budget should be spent only where exact proof or edit readiness requires it.

Structural budget should be used to preserve navigation shape and provenance.

## Omission Rules

Omission is part of the design, not an accident.

Every omitted candidate should be classifiable with a reason such as:

- insufficient priority
- superseded by a higher representation level
- duplicated by another item
- stale relative to current frontier
- budget overflow
- filtered by strategy policy
- filtered by class or visibility policy

Omission reasons should be preserved in the turn trace.

## Compaction Rules

Compaction should reduce context load without destroying useful state.

Compaction candidates include:

- stale tool results
- prior graph summaries
- earlier exploration notes
- repeated history items

Compaction should preserve:

- what was compacted
- why it was compacted
- what summary replaced it
- how much budget was recovered

Section allocation caps and history compaction policy should be driven by the active strategy config, not hidden prompt-assembly constants.

## Recommended Render Shape

The exact text format may evolve, but the shape should remain stable and inspectable.

A rendered context should make section boundaries explicit.

For example, each section should be clearly labeled in a consistent way so that:

- humans can inspect it
- hashes are stable under canonical rendering
- replay can reconstruct the exact same bytes

## Example Conceptual Layout

An illustrative conceptual layout:

```text
[base_runtime_instructions]
...

[response_contract]
...

[objective_state]
task=...
task_class=...
readiness=...
unresolved_questions=...

[selected_history]
...

[active_code_windows]
...

[code_navigation]
...

[tool_contracts]
...
```

This is not necessarily the final literal format, but it illustrates the intended sectioning.

## What Should Not Happen

The context window should not:

- dump the full transcript by default
- dump the full file graph by default
- dump full edited code into history by default
- mix hidden and explicit state
- include unvalidated raw model payloads as trusted context
- silently trim important evidence without recording omission
- vary section ordering arbitrarily between runs

## Reconstruction Requirements

For every rendered context window, GraphBench must be able to reconstruct:

- selected source objects
- omitted candidates
- representation levels
- section order
- rendered contents
- byte and token counts
- rendered context hash
- rendered prompt hash when combined with non-context prompt sections

## Relationship to Other Docs

This document focuses on the rendered prompt-visible window.

Related docs:

- `codegraph.md`
  - durable graph and session substrate
- `harness.md`
  - runtime loop and prompt assembly owner
- `context-tracing-and-observability.md`
  - tracing, turn ledger, and exact reconstruction requirements
- `scoring-and-evaluation.md`
  - how context quality is evaluated
