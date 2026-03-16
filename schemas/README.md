# Schemas

Versioned artifact contracts live here.

The first schema set covers:

- fixture manifests
- task specs
- evidence specs
- strategy configs
- context objects
- context-window sections
- turn traces
- score reports
- run manifests

Lockstep policy:

- Rust types live in `crates/graphbench-core/src/artifacts.rs` and `crates/graphbench-core/src/strategy.rs`
- frontend types live in `frontend/src/types/artifacts.ts`
- both are kept in manual lockstep with the schema files in this directory

Versioning details live in `schemas/versioning.md`.
