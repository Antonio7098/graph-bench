pub mod artifacts;
pub mod error;
pub mod fixtures;
pub mod graph;
pub mod logging;
pub mod strategy;
pub mod tasks;

pub use artifacts::{
    AcceptableProof, ContextIdentity, ContextObject, ContextObjectHashSet, ContextObjectKind,
    ContextProvenance, Difficulty, EvidenceFact, EvidenceMatch, EvidenceSpec, FixtureLanguage,
    FixtureManifest, FixtureMetadata, GraphSnapshot, LeaseState, MirrorPolicy, OmittedCandidate,
    ProofKind, ReadinessState, RelevanceScore, RenderedContextSection, RepositoryRef,
    RepresentationLevel, RunManifest, RunSchemaVersionSet, ScoreMetrics, ScoreReport, ScoreValue,
    TaskClass, TaskReview, TaskSpec, TelemetryCounts, TurnHashSet, TurnRequest, TurnResponse,
    TurnSelection, TurnTrace, VerificationTarget,
};
pub use error::{AppError, ErrorCategory, ErrorCode, ErrorContext};
pub use fixtures::{
    FixtureRepository, FixtureResolution, load_fixture_manifest, load_fixture_manifests,
};
pub use graph::{GraphPromptHooks, GraphSession, GraphWorkspace, persist_codegraph_snapshot};
pub use logging::{LogEvent, LogField};
pub use strategy::{
    ContextWindowCompactionPolicy, ContextWindowStrategyPolicy, GraphDiscoveryMode, ProjectionMode,
    RereadMode, STRATEGY_CONFIG_SCHEMA_VERSION, SectionTrimDirection, StrategyConfig,
    StrategySectionBudget,
};
pub use tasks::{
    CorpusSummary, EvidenceMatchResult, TaskCorpus, load_evidence_spec, load_evidence_specs,
    load_task_corpus, load_task_spec, load_task_specs, match_proof,
};
