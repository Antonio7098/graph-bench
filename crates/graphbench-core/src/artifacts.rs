use crate::error::{AppError, ErrorCode, ErrorContext};
use crate::strategy::{STRATEGY_CONFIG_SCHEMA_VERSION, StrategyConfig, validate_strategy_id};
use serde::{Deserialize, Serialize};

pub const FIXTURE_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const TASK_SPEC_SCHEMA_VERSION: u32 = 1;
pub const EVIDENCE_SPEC_SCHEMA_VERSION: u32 = 1;
pub const CONTEXT_OBJECT_SCHEMA_VERSION: u32 = 1;
pub const CONTEXT_WINDOW_SECTION_SCHEMA_VERSION: u32 = 1;
pub const TURN_TRACE_SCHEMA_VERSION: u32 = 1;
pub const SCORE_REPORT_SCHEMA_VERSION: u32 = 1;
pub const RUN_MANIFEST_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MirrorPolicy {
    Workspace,
    LocalCacheOnly,
    MirrorRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryRef {
    pub source: String,
    pub commit_sha: String,
    pub mirror_policy: MirrorPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphSnapshot {
    pub snapshot_id: String,
    pub snapshot_format: String,
    pub snapshot_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureMetadata {
    pub title: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FixtureLanguage {
    Rust,
    TypeScript,
    Markdown,
    Yaml,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureManifest {
    pub fixture_id: String,
    pub schema_version: u32,
    pub repository: RepositoryRef,
    pub graph: GraphSnapshot,
    pub languages: Vec<FixtureLanguage>,
    pub metadata: FixtureMetadata,
}

impl FixtureManifest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != FIXTURE_MANIFEST_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "fixture_manifest",
                "validate",
                "unsupported fixture schema version",
            ));
        }

        validate_identifier(&self.fixture_id, "fixture.fixture_id")?;
        validate_commit_sha(&self.repository.commit_sha)?;
        validate_non_empty(&self.repository.source, "fixture.repository.source")?;
        validate_non_empty(&self.graph.snapshot_format, "fixture.graph.snapshot_format")?;
        validate_non_empty(&self.graph.snapshot_ref, "fixture.graph.snapshot_ref")?;
        validate_snapshot_id(&self.graph.snapshot_id)?;
        validate_non_empty(&self.metadata.title, "fixture.metadata.title")?;

        if self.languages.is_empty() {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "fixture_manifest",
                "validate",
                "fixture.languages must not be empty",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    Locate,
    Explain,
    PrepareToEdit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationTarget {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskReview {
    pub required_evidence_is_sufficient: bool,
    pub distractors_are_realistic: bool,
    pub multiple_valid_paths_considered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskSpec {
    pub task_id: String,
    pub schema_version: u32,
    pub fixture_id: String,
    pub title: String,
    pub statement: String,
    pub task_class: TaskClass,
    pub difficulty: Difficulty,
    pub allowed_tools: Vec<String>,
    pub turn_budget: u32,
    pub evidence_spec_ref: String,
    pub seed_paths: Vec<String>,
    pub seed_selectors: Vec<String>,
    pub verification_targets: Vec<VerificationTarget>,
    pub known_distractor_regions: Vec<String>,
    pub expected_edit_loci: Vec<String>,
    pub review: TaskReview,
}

impl TaskSpec {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != TASK_SPEC_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "task_spec",
                "validate",
                "unsupported task schema version",
            ));
        }

        validate_identifier(&self.task_id, "task.task_id")?;
        validate_identifier(&self.fixture_id, "task.fixture_id")?;
        validate_non_empty(&self.title, "task.title")?;
        validate_non_empty(&self.statement, "task.statement")?;
        validate_non_empty(&self.evidence_spec_ref, "task.evidence_spec_ref")?;

        if self.allowed_tools.is_empty() {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "task_spec",
                "validate",
                "task.allowed_tools must not be empty",
            ));
        }

        if self.turn_budget == 0 {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "task_spec",
                "validate",
                "task.turn_budget must be greater than zero",
            ));
        }

        if self.verification_targets.is_empty() {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "task_spec",
                "validate",
                "task.verification_targets must not be empty",
            ));
        }

        if self.seed_paths.is_empty() && self.seed_selectors.is_empty() {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "task_spec",
                "validate",
                "task must expose at least one seed path or selector",
            ));
        }

        if !(self.review.required_evidence_is_sufficient
            && self.review.distractors_are_realistic
            && self.review.multiple_valid_paths_considered)
        {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "task_spec",
                "validate",
                "task review must explicitly pass all authoring checks",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProofKind {
    Path,
    Symbol,
    LogicalKey,
    Excerpt,
    GraphPath,
    Coderef,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptableProof {
    pub kind: ProofKind,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceFact {
    pub fact_id: String,
    pub description: String,
    pub acceptable_proofs: Vec<AcceptableProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceSpec {
    pub evidence_spec_id: String,
    pub schema_version: u32,
    pub required_facts: Vec<EvidenceFact>,
    pub supporting_facts: Vec<EvidenceFact>,
    pub distractor_facts: Vec<EvidenceFact>,
    pub verification_targets: Vec<VerificationTarget>,
}

impl EvidenceSpec {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != EVIDENCE_SPEC_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "evidence_spec",
                "validate",
                "unsupported evidence schema version",
            ));
        }

        validate_identifier(&self.evidence_spec_id, "evidence.evidence_spec_id")?;

        if self.required_facts.is_empty() {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "evidence_spec",
                "validate",
                "evidence.required_facts must not be empty",
            ));
        }

        if self.distractor_facts.is_empty() {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "evidence_spec",
                "validate",
                "evidence.distractor_facts must not be empty",
            ));
        }

        if self.verification_targets.is_empty() {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "evidence_spec",
                "validate",
                "evidence.verification_targets must not be empty",
            ));
        }

        for fact in self
            .required_facts
            .iter()
            .chain(self.supporting_facts.iter())
            .chain(self.distractor_facts.iter())
        {
            validate_identifier(&fact.fact_id, "evidence.fact_id")?;
            validate_non_empty(&fact.description, "evidence.description")?;

            if fact.acceptable_proofs.is_empty() {
                return Err(validation_error(
                    ErrorCode::SchemaValidationFailed,
                    "evidence_spec",
                    "validate",
                    "every fact must declare acceptable proofs",
                ));
            }

            for proof in &fact.acceptable_proofs {
                validate_non_empty(&proof.value, "evidence.acceptable_proof.value")?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextObjectKind {
    Symbol,
    SymbolCluster,
    FileRegion,
    DependencyNeighborhood,
    TestTarget,
    ValidationArtifact,
    ToolResultSummary,
    DirtyEditLocus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextIdentity {
    pub logical_key: Option<String>,
    pub path: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextProvenance {
    pub source_kind: String,
    pub anchor_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
    L5,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    Granted,
    Expiring,
    Denied,
}

pub type RelevanceScore = i32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceMatch {
    pub fact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextObjectHashSet {
    pub object_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextObject {
    pub context_object_id: String,
    pub schema_version: u32,
    pub graph_snapshot_id: String,
    pub kind: ContextObjectKind,
    pub identity: ContextIdentity,
    pub representation_level: RepresentationLevel,
    pub provenance: ContextProvenance,
    pub relevance_score: RelevanceScore,
    pub lease_state: LeaseState,
    pub evidence_matches: Vec<EvidenceMatch>,
    pub hashes: ContextObjectHashSet,
}

impl ContextObject {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != CONTEXT_OBJECT_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "context_object",
                "validate",
                "unsupported context object schema version",
            ));
        }

        validate_identifier(&self.context_object_id, "context.context_object_id")?;
        validate_snapshot_id(&self.graph_snapshot_id)?;
        validate_non_empty(
            &self.provenance.source_kind,
            "context.provenance.source_kind",
        )?;
        validate_non_empty(&self.provenance.anchor_id, "context.provenance.anchor_id")?;
        validate_hash(&self.hashes.object_hash, "context.hashes.object_hash")?;

        if self.identity.logical_key.is_none()
            && self.identity.path.is_none()
            && self.identity.symbol.is_none()
        {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "context_object",
                "validate",
                "context identity must include at least one of logical_key, path, or symbol",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderedContextSection {
    pub section_id: String,
    pub schema_version: u32,
    pub title: String,
    pub content: String,
    pub byte_count: u32,
    pub token_count: u32,
}

impl RenderedContextSection {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != CONTEXT_WINDOW_SECTION_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "context_window_section",
                "validate",
                "unsupported context window section schema version",
            ));
        }

        validate_identifier(&self.section_id, "context_window_section.section_id")?;
        validate_non_empty(&self.title, "context_window_section.title")?;
        validate_non_empty(&self.content, "context_window_section.content")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnRequest {
    pub schema_version: u32,
    pub prompt_version: String,
    pub prompt_hash: String,
    pub context_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnResponse {
    pub provider: String,
    pub model_slug: String,
    pub schema_version: u32,
    pub validated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OmittedCandidate {
    pub candidate_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnSelection {
    pub selected_context_objects: Vec<String>,
    pub omitted_candidates: Vec<OmittedCandidate>,
    pub rendered_sections: Vec<RenderedContextSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryCounts {
    pub prompt_bytes: u32,
    pub prompt_tokens: u32,
    pub latency_ms: u32,
    pub tool_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    NotReady,
    EvidenceVisible,
    EvidenceAcquired,
    ReadyToEdit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnHashSet {
    pub turn_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurnTrace {
    pub run_id: String,
    pub turn_index: u32,
    pub task_id: String,
    pub fixture_id: String,
    pub strategy_id: String,
    pub request: TurnRequest,
    pub response: TurnResponse,
    pub selection: TurnSelection,
    pub telemetry: TelemetryCounts,
    pub evidence_delta: Vec<String>,
    pub readiness_state: ReadinessState,
    pub readiness_reason: String,
    pub hashes: TurnHashSet,
}

impl TurnTrace {
    pub fn validate_for_creation(&self) -> Result<(), AppError> {
        self.validate_common("create")
    }

    pub fn validate_for_persistence(&self) -> Result<(), AppError> {
        self.validate_common("persist")
    }

    pub fn validate_for_replay(&self) -> Result<(), AppError> {
        self.validate_common("replay")
    }

    fn validate_common(&self, operation: &'static str) -> Result<(), AppError> {
        validate_identifier(&self.run_id, "turn_trace.run_id")?;
        validate_identifier(&self.task_id, "turn_trace.task_id")?;
        validate_identifier(&self.fixture_id, "turn_trace.fixture_id")?;
        validate_strategy_id(&self.strategy_id)?;
        validate_hash(&self.request.prompt_hash, "turn_trace.request.prompt_hash")?;
        validate_hash(
            &self.request.context_hash,
            "turn_trace.request.context_hash",
        )?;
        validate_non_empty(
            &self.request.prompt_version,
            "turn_trace.request.prompt_version",
        )?;
        validate_non_empty(&self.response.provider, "turn_trace.response.provider")?;
        validate_non_empty(&self.response.model_slug, "turn_trace.response.model_slug")?;
        validate_non_empty(&self.readiness_reason, "turn_trace.readiness_reason")?;
        validate_hash(&self.hashes.turn_hash, "turn_trace.hashes.turn_hash")?;

        if self.request.schema_version != TURN_TRACE_SCHEMA_VERSION
            || self.response.schema_version != TURN_TRACE_SCHEMA_VERSION
        {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "turn_trace",
                operation,
                "turn request and response schema versions must match the turn trace schema version",
            ));
        }

        if !self.response.validated {
            return Err(validation_error(
                ErrorCode::ProviderResponseInvalid,
                "turn_trace",
                operation,
                "turn response must be validated before use",
            ));
        }

        for section in &self.selection.rendered_sections {
            section.validate()?;
        }

        Ok(())
    }
}

pub type ScoreValue = f64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreMetrics {
    pub required_evidence_recall: ScoreValue,
    pub evidence_precision: ScoreValue,
    pub irrelevant_material_ratio: ScoreValue,
    pub turns_to_readiness: u32,
    pub reread_count: u32,
    pub post_readiness_drift_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreReport {
    pub run_id: String,
    pub task_id: String,
    pub schema_version: u32,
    pub evidence_visibility_score: ScoreValue,
    pub evidence_acquisition_score: ScoreValue,
    pub evidence_efficiency_score: ScoreValue,
    pub explanation_quality_score: ScoreValue,
    pub metrics: ScoreMetrics,
}

impl ScoreReport {
    pub fn validate_for_creation(&self) -> Result<(), AppError> {
        self.validate_common("create")
    }

    pub fn validate_for_persistence(&self) -> Result<(), AppError> {
        self.validate_common("persist")
    }

    pub fn validate_for_scoring(&self) -> Result<(), AppError> {
        self.validate_common("score")
    }

    fn validate_common(&self, operation: &'static str) -> Result<(), AppError> {
        if self.schema_version != SCORE_REPORT_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "score_report",
                operation,
                "unsupported score report schema version",
            ));
        }

        validate_identifier(&self.run_id, "score_report.run_id")?;
        validate_identifier(&self.task_id, "score_report.task_id")?;

        for score in [
            self.evidence_visibility_score,
            self.evidence_acquisition_score,
            self.evidence_efficiency_score,
            self.explanation_quality_score,
            self.metrics.required_evidence_recall,
            self.metrics.evidence_precision,
            self.metrics.irrelevant_material_ratio,
        ] {
            if !(0.0..=1.0).contains(&score) {
                return Err(validation_error(
                    ErrorCode::SchemaValidationFailed,
                    "score_report",
                    operation,
                    "all normalized scores and ratios must be between 0.0 and 1.0",
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSchemaVersionSet {
    pub fixture_manifest: u32,
    pub task_spec: u32,
    pub evidence_spec: u32,
    pub strategy_config: u32,
    pub context_object: u32,
    pub context_window_section: u32,
    pub turn_trace: u32,
    pub score_report: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunManifest {
    pub run_id: String,
    pub schema_version: u32,
    pub fixture_id: String,
    pub task_id: String,
    pub strategy_id: String,
    pub strategy_config: StrategyConfig,
    pub harness_version: String,
    pub schema_version_set: RunSchemaVersionSet,
    pub provider: String,
    pub model_slug: String,
    pub prompt_version: String,
    pub graph_snapshot_id: String,
    pub started_at: String,
    pub completed_at: String,
    pub outcome: String,
}

impl RunManifest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != RUN_MANIFEST_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "run_manifest",
                "validate",
                "unsupported run manifest schema version",
            ));
        }

        validate_identifier(&self.run_id, "run_manifest.run_id")?;
        validate_identifier(&self.fixture_id, "run_manifest.fixture_id")?;
        validate_identifier(&self.task_id, "run_manifest.task_id")?;
        validate_strategy_id(&self.strategy_id)?;
        self.strategy_config.validate()?;
        if self.strategy_config.strategy_id != self.strategy_id {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "run_manifest",
                "validate",
                "run manifest strategy_id must match strategy_config.strategy_id",
            ));
        }
        if self.schema_version_set.strategy_config != STRATEGY_CONFIG_SCHEMA_VERSION {
            return Err(validation_error(
                ErrorCode::SchemaValidationFailed,
                "run_manifest",
                "validate",
                "run manifest strategy_config schema version must match the current strategy config schema version",
            ));
        }
        validate_non_empty(&self.harness_version, "run_manifest.harness_version")?;
        validate_non_empty(&self.provider, "run_manifest.provider")?;
        validate_non_empty(&self.model_slug, "run_manifest.model_slug")?;
        validate_non_empty(&self.prompt_version, "run_manifest.prompt_version")?;
        validate_snapshot_id(&self.graph_snapshot_id)?;
        validate_non_empty(&self.started_at, "run_manifest.started_at")?;
        validate_non_empty(&self.completed_at, "run_manifest.completed_at")?;
        validate_non_empty(&self.outcome, "run_manifest.outcome")?;
        Ok(())
    }
}

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(validation_error(
            ErrorCode::SchemaValidationFailed,
            "artifacts",
            "validate_non_empty",
            field,
        ));
    }

    Ok(())
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), AppError> {
    validate_non_empty(value, field)?;

    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(validation_error(
            ErrorCode::SchemaValidationFailed,
            "artifacts",
            "validate_identifier",
            field,
        ));
    }

    Ok(())
}

fn validate_commit_sha(value: &str) -> Result<(), AppError> {
    if value.len() != 40 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(validation_error(
            ErrorCode::FixtureManifestInvalid,
            "fixture_manifest",
            "validate_commit_sha",
            "fixture.repository.commit_sha must be a 40-character hex SHA",
        ));
    }

    Ok(())
}

fn validate_snapshot_id(value: &str) -> Result<(), AppError> {
    let Some(hash) = value.strip_prefix("sha256:") else {
        return Err(validation_error(
            ErrorCode::SchemaValidationFailed,
            "artifacts",
            "validate_snapshot_id",
            "snapshot ids must use sha256:<hex> format",
        ));
    };

    if hash.len() != 64 || !hash.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(validation_error(
            ErrorCode::SchemaValidationFailed,
            "artifacts",
            "validate_snapshot_id",
            "snapshot ids must use sha256:<64 hex> format",
        ));
    }

    Ok(())
}

fn validate_hash(value: &str, field: &'static str) -> Result<(), AppError> {
    validate_snapshot_id(value).map_err(|_| {
        validation_error(
            ErrorCode::SchemaValidationFailed,
            "artifacts",
            "validate_hash",
            field,
        )
    })
}

fn validation_error(
    code: ErrorCode,
    component: &'static str,
    operation: &'static str,
    message: &'static str,
) -> AppError {
    AppError::new(
        code,
        message,
        ErrorContext {
            component,
            operation,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        CONTEXT_WINDOW_SECTION_SCHEMA_VERSION, Difficulty, EVIDENCE_SPEC_SCHEMA_VERSION,
        FIXTURE_MANIFEST_SCHEMA_VERSION, FixtureLanguage, FixtureManifest, FixtureMetadata,
        GraphSnapshot, MirrorPolicy, RenderedContextSection, RepositoryRef,
        SCORE_REPORT_SCHEMA_VERSION, ScoreMetrics, ScoreReport, TASK_SPEC_SCHEMA_VERSION,
        TURN_TRACE_SCHEMA_VERSION, TaskClass, TaskReview, TaskSpec, TurnHashSet, TurnRequest,
        TurnResponse, TurnSelection, TurnTrace, VerificationTarget,
    };

    #[test]
    fn fixture_validation_requires_pinned_sha() {
        let fixture = FixtureManifest {
            fixture_id: "graphbench.internal".to_owned(),
            schema_version: FIXTURE_MANIFEST_SCHEMA_VERSION,
            repository: RepositoryRef {
                source: ".".to_owned(),
                commit_sha: "abc".to_owned(),
                mirror_policy: MirrorPolicy::Workspace,
            },
            graph: GraphSnapshot {
                snapshot_id: format!("sha256:{}", "a".repeat(64)),
                snapshot_format: "json".to_owned(),
                snapshot_ref: "snapshot.json".to_owned(),
            },
            languages: vec![FixtureLanguage::Rust],
            metadata: FixtureMetadata {
                title: "Fixture".to_owned(),
                notes: String::new(),
            },
        };

        assert!(fixture.validate().is_err());
    }

    #[test]
    fn task_validation_requires_review_and_verification_targets() {
        let task = TaskSpec {
            task_id: "task-1".to_owned(),
            schema_version: TASK_SPEC_SCHEMA_VERSION,
            fixture_id: "fixture-1".to_owned(),
            title: "Task".to_owned(),
            statement: "Collect evidence.".to_owned(),
            task_class: TaskClass::PrepareToEdit,
            difficulty: Difficulty::Medium,
            allowed_tools: vec!["read_file".to_owned()],
            turn_budget: 5,
            evidence_spec_ref: "evidence.json".to_owned(),
            seed_paths: vec!["README.md".to_owned()],
            seed_selectors: Vec::new(),
            verification_targets: Vec::new(),
            known_distractor_regions: Vec::new(),
            expected_edit_loci: Vec::new(),
            review: TaskReview {
                required_evidence_is_sufficient: true,
                distractors_are_realistic: true,
                multiple_valid_paths_considered: true,
            },
        };

        assert!(task.validate().is_err());
    }

    #[test]
    fn turn_trace_validation_rejects_unvalidated_responses() {
        let trace = TurnTrace {
            run_id: "run-1".to_owned(),
            turn_index: 0,
            task_id: "task-1".to_owned(),
            fixture_id: "fixture-1".to_owned(),
            strategy_id: "baseline".to_owned(),
            request: TurnRequest {
                schema_version: TURN_TRACE_SCHEMA_VERSION,
                prompt_version: "v1".to_owned(),
                prompt_hash: format!("sha256:{}", "b".repeat(64)),
                context_hash: format!("sha256:{}", "c".repeat(64)),
            },
            response: TurnResponse {
                provider: "test".to_owned(),
                model_slug: "model".to_owned(),
                schema_version: TURN_TRACE_SCHEMA_VERSION,
                validated: false,
            },
            selection: TurnSelection {
                selected_context_objects: vec!["context-1".to_owned()],
                omitted_candidates: Vec::new(),
                rendered_sections: vec![RenderedContextSection {
                    section_id: "objective_state".to_owned(),
                    schema_version: CONTEXT_WINDOW_SECTION_SCHEMA_VERSION,
                    title: "Objective State".to_owned(),
                    content: "Task body".to_owned(),
                    byte_count: 9,
                    token_count: 2,
                }],
            },
            telemetry: super::TelemetryCounts {
                prompt_bytes: 9,
                prompt_tokens: 2,
                latency_ms: 10,
                tool_calls: 1,
            },
            evidence_delta: vec!["fact-1".to_owned()],
            readiness_state: super::ReadinessState::EvidenceAcquired,
            readiness_reason: "required facts were gathered".to_owned(),
            hashes: TurnHashSet {
                turn_hash: format!("sha256:{}", "d".repeat(64)),
            },
        };

        assert!(trace.validate_for_creation().is_err());
    }

    #[test]
    fn score_report_validation_bounds_scores() {
        let report = ScoreReport {
            run_id: "run-1".to_owned(),
            task_id: "task-1".to_owned(),
            schema_version: SCORE_REPORT_SCHEMA_VERSION,
            evidence_visibility_score: 1.0,
            evidence_acquisition_score: 0.9,
            evidence_efficiency_score: 0.8,
            explanation_quality_score: 1.2,
            metrics: ScoreMetrics {
                required_evidence_recall: 1.0,
                evidence_precision: 0.9,
                irrelevant_material_ratio: 0.1,
                turns_to_readiness: 2,
                reread_count: 0,
                post_readiness_drift_turns: 0,
            },
        };

        assert!(report.validate_for_scoring().is_err());
    }

    #[test]
    fn rendered_context_section_validates() {
        let section = RenderedContextSection {
            section_id: "selected_history".to_owned(),
            schema_version: CONTEXT_WINDOW_SECTION_SCHEMA_VERSION,
            title: "Selected History".to_owned(),
            content: "Recent items".to_owned(),
            byte_count: 12,
            token_count: 3,
        };

        assert!(section.validate().is_ok());
    }

    #[test]
    fn evidence_spec_validation_requires_facts() {
        let spec = super::EvidenceSpec {
            evidence_spec_id: "evidence-1".to_owned(),
            schema_version: EVIDENCE_SPEC_SCHEMA_VERSION,
            required_facts: Vec::new(),
            supporting_facts: Vec::new(),
            distractor_facts: Vec::new(),
            verification_targets: vec![VerificationTarget {
                kind: "command".to_owned(),
                value: "cargo test".to_owned(),
            }],
        };

        assert!(spec.validate().is_err());
    }
}
