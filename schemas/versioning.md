# Schema Versioning Strategy

GraphBench versions each persisted artifact family independently.

Current policy:

- schema files live in `schemas/`
- each artifact carries its own `schema_version`
- Rust types in `graphbench-core::artifacts` are kept in manual lockstep with these schema files
- frontend types in `frontend/src/types/artifacts.ts` are kept in manual lockstep with the same source schemas
- breaking changes require a schema version increment for the affected artifact family
- run metadata records the version set used for a run

Validation is required:

- when an artifact is created
- before persistence
- before replay
- before scoring
- before UI projection
