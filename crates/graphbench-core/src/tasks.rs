use crate::artifacts::{AcceptableProof, EvidenceFact, EvidenceSpec, TaskSpec};
use crate::error::{AppError, ErrorCode, ErrorContext};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceMatchResult {
    pub fact_id: String,
    pub matched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCorpus {
    pub tasks: Vec<TaskSpec>,
    pub evidence_specs: Vec<EvidenceSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusSummary {
    pub fixture_ids: BTreeSet<String>,
    pub task_count: usize,
    pub evidence_spec_count: usize,
}

pub fn match_proof(fact: &EvidenceFact, candidate: &AcceptableProof) -> EvidenceMatchResult {
    let matched = fact
        .acceptable_proofs
        .iter()
        .any(|proof| proof.kind == candidate.kind && proof.value == candidate.value);

    EvidenceMatchResult {
        fact_id: fact.fact_id.clone(),
        matched,
    }
}

pub fn load_task_spec(path: impl AsRef<Path>) -> Result<TaskSpec, AppError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| {
        AppError::with_source(
            ErrorCode::SchemaValidationFailed,
            format!("failed to read task spec at {}", path.display()),
            ErrorContext {
                component: "tasks",
                operation: "read_task_spec",
            },
            source,
        )
    })?;

    let task: TaskSpec = serde_json::from_str(&raw).map_err(|source| {
        AppError::with_source(
            ErrorCode::SchemaValidationFailed,
            format!("failed to parse task spec at {}", path.display()),
            ErrorContext {
                component: "tasks",
                operation: "parse_task_spec",
            },
            source,
        )
    })?;

    task.validate()?;
    Ok(task)
}

pub fn load_evidence_spec(path: impl AsRef<Path>) -> Result<EvidenceSpec, AppError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| {
        AppError::with_source(
            ErrorCode::SchemaValidationFailed,
            format!("failed to read evidence spec at {}", path.display()),
            ErrorContext {
                component: "tasks",
                operation: "read_evidence_spec",
            },
            source,
        )
    })?;

    let evidence: EvidenceSpec = serde_json::from_str(&raw).map_err(|source| {
        AppError::with_source(
            ErrorCode::SchemaValidationFailed,
            format!("failed to parse evidence spec at {}", path.display()),
            ErrorContext {
                component: "tasks",
                operation: "parse_evidence_spec",
            },
            source,
        )
    })?;

    evidence.validate()?;
    Ok(evidence)
}

pub fn load_task_specs(root: impl AsRef<Path>) -> Result<Vec<TaskSpec>, AppError> {
    load_specs(root.as_ref(), "task.json", load_task_spec)
}

pub fn load_evidence_specs(root: impl AsRef<Path>) -> Result<Vec<EvidenceSpec>, AppError> {
    load_specs(root.as_ref(), "evidence.json", load_evidence_spec)
}

pub fn load_task_corpus(root: impl AsRef<Path>) -> Result<TaskCorpus, AppError> {
    let tasks = load_task_specs(root.as_ref())?;
    let evidence_specs = load_evidence_specs(root.as_ref())?;
    validate_corpus(&tasks, &evidence_specs)?;
    Ok(TaskCorpus {
        tasks,
        evidence_specs,
    })
}

impl TaskCorpus {
    pub fn summary(&self) -> CorpusSummary {
        CorpusSummary {
            fixture_ids: self
                .tasks
                .iter()
                .map(|task| task.fixture_id.clone())
                .collect::<BTreeSet<_>>(),
            task_count: self.tasks.len(),
            evidence_spec_count: self.evidence_specs.len(),
        }
    }
}

fn validate_corpus(tasks: &[TaskSpec], evidence_specs: &[EvidenceSpec]) -> Result<(), AppError> {
    if tasks.len() < 10 {
        return Err(AppError::new(
            ErrorCode::SchemaValidationFailed,
            "the initial task corpus must contain at least 10 tasks",
            ErrorContext {
                component: "tasks",
                operation: "validate_corpus",
            },
        ));
    }

    let evidence_by_id = evidence_specs
        .iter()
        .map(|spec| (spec.evidence_spec_id.clone(), spec))
        .collect::<BTreeMap<_, _>>();

    for task in tasks {
        let evidence = evidence_by_id.get(&task.evidence_spec_ref).ok_or_else(|| {
            AppError::new(
                ErrorCode::SchemaValidationFailed,
                format!(
                    "task '{}' references missing evidence spec '{}'",
                    task.task_id, task.evidence_spec_ref
                ),
                ErrorContext {
                    component: "tasks",
                    operation: "validate_corpus_evidence_ref",
                },
            )
        })?;

        validate_task_against_evidence(task, evidence)?;
    }

    Ok(())
}

fn validate_task_against_evidence(
    task: &TaskSpec,
    evidence: &EvidenceSpec,
) -> Result<(), AppError> {
    if evidence.required_facts.len() < 2 {
        return Err(AppError::new(
            ErrorCode::SchemaValidationFailed,
            format!("task '{}' needs at least two required facts", task.task_id),
            ErrorContext {
                component: "tasks",
                operation: "validate_required_fact_count",
            },
        ));
    }

    if task.known_distractor_regions.is_empty() {
        return Err(AppError::new(
            ErrorCode::SchemaValidationFailed,
            format!("task '{}' must declare distractor regions", task.task_id),
            ErrorContext {
                component: "tasks",
                operation: "validate_distractor_regions",
            },
        ));
    }

    if task.seed_paths.len() + task.seed_selectors.len() < 2 {
        return Err(AppError::new(
            ErrorCode::SchemaValidationFailed,
            format!(
                "task '{}' must offer multiple traversal entry points",
                task.task_id
            ),
            ErrorContext {
                component: "tasks",
                operation: "validate_multiple_paths",
            },
        ));
    }

    for target in &task.verification_targets {
        if !evidence.verification_targets.contains(target) {
            return Err(AppError::new(
                ErrorCode::SchemaValidationFailed,
                format!(
                    "task '{}' verification target '{}' must appear in the evidence spec",
                    task.task_id, target.value
                ),
                ErrorContext {
                    component: "tasks",
                    operation: "validate_verification_targets",
                },
            ));
        }
    }

    Ok(())
}

fn load_specs<T>(
    root: &Path,
    suffix: &str,
    loader: fn(PathBuf) -> Result<T, AppError>,
) -> Result<Vec<T>, AppError> {
    let mut specs = Vec::new();
    collect_specs(root, suffix, &mut specs, loader)?;
    Ok(specs)
}

fn collect_specs<T>(
    root: &Path,
    suffix: &str,
    specs: &mut Vec<T>,
    loader: fn(PathBuf) -> Result<T, AppError>,
) -> Result<(), AppError> {
    for entry in fs::read_dir(root).map_err(|source| {
        AppError::with_source(
            ErrorCode::SchemaValidationFailed,
            format!("failed to read directory {}", root.display()),
            ErrorContext {
                component: "tasks",
                operation: "read_directory",
            },
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            AppError::with_source(
                ErrorCode::SchemaValidationFailed,
                format!("failed to read directory entry under {}", root.display()),
                ErrorContext {
                    component: "tasks",
                    operation: "read_directory_entry",
                },
                source,
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_specs(&path, suffix, specs, loader)?;
        } else if path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|name| name.ends_with(suffix))
        {
            specs.push(loader(path)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_task_corpus, load_task_specs, match_proof};
    use crate::artifacts::{AcceptableProof, ProofKind};
    use std::path::Path;

    #[test]
    fn proof_matching_is_exact_by_kind_and_value() {
        let fact = crate::artifacts::EvidenceFact {
            fact_id: "fact-1".to_owned(),
            description: "A file must exist".to_owned(),
            acceptable_proofs: vec![AcceptableProof {
                kind: ProofKind::Path,
                value: "README.md".to_owned(),
            }],
        };

        let matched = match_proof(
            &fact,
            &AcceptableProof {
                kind: ProofKind::Path,
                value: "README.md".to_owned(),
            },
        );

        assert!(matched.matched);
    }

    #[test]
    fn initial_task_corpus_loads_and_validates() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tasks");
        if !root.exists() {
            return;
        }
        let corpus = load_task_corpus(root).expect("task corpus should validate");
        let summary = corpus.summary();
        assert_eq!(summary.task_count, 10);
        assert_eq!(summary.evidence_spec_count, 10);
        assert!(summary.fixture_ids.contains("graphbench.internal"));
    }

    #[test]
    fn task_specs_load_in_stable_count() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tasks");
        if !root.exists() {
            return;
        }
        let tasks = load_task_specs(root).expect("task specs should load");
        assert_eq!(tasks.len(), 10);
    }
}
