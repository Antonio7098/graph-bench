use crate::tools::ToolCallTrace;
use crate::turn_ledger::TurnLedger;
use graphbench_core::artifacts::{
    AcceptableProof, EvidenceFact, EvidenceSpec, ProofKind, ReadinessState, ScoreMetrics,
    ScoreReport, SCORE_REPORT_SCHEMA_VERSION,
};
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactRole {
    Required,
    Supporting,
    Distractor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceChannel {
    VisibleContext,
    ToolTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofObservation {
    pub turn_index: u32,
    pub proof_kind: ProofKind,
    pub proof_value: String,
    pub channel: EvidenceChannel,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactScore {
    pub fact_id: String,
    pub role: FactRole,
    pub visible: bool,
    pub acquired: bool,
    pub first_visible_turn: Option<u32>,
    pub first_acquired_turn: Option<u32>,
    pub observations: Vec<ProofObservation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplanationQualityScore {
    pub score: f64,
    pub rationale: String,
}

pub trait ExplanationQualityScorer {
    fn score(
        &self,
        input: &ExplanationQualityInput<'_>,
    ) -> Result<ExplanationQualityScore, AppError>;
}

#[derive(Debug, Default)]
pub struct UnscoredExplanationQuality;

impl ExplanationQualityScorer for UnscoredExplanationQuality {
    fn score(
        &self,
        _input: &ExplanationQualityInput<'_>,
    ) -> Result<ExplanationQualityScore, AppError> {
        Ok(ExplanationQualityScore {
            score: 0.0,
            rationale:
                "Explanation quality was not scored; deterministic evidence accounting only."
                    .to_owned(),
        })
    }
}

#[derive(Debug)]
pub struct ExplanationQualityInput<'a> {
    pub ledger: &'a TurnLedger,
    pub evidence_spec: &'a EvidenceSpec,
    pub deterministic: &'a DeterministicScoreBreakdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeAssistedSynthesisScore {
    pub score: f64,
    pub rationale: String,
    pub readiness_correctness: Option<f64>,
    pub unsupported_claim_ratio: Option<f64>,
}

pub trait JudgeAssistedSynthesisScorer {
    fn score(
        &self,
        input: &JudgeAssistedSynthesisInput<'_>,
    ) -> Result<JudgeAssistedSynthesisScore, AppError>;
}

#[derive(Debug)]
pub struct JudgeAssistedSynthesisInput<'a> {
    pub ledger: &'a TurnLedger,
    pub evidence_spec: &'a EvidenceSpec,
    pub deterministic: &'a DeterministicScoreBreakdown,
    pub explanation_quality: &'a ExplanationQualityScore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeterministicScoreBreakdown {
    pub report: ScoreReport,
    pub explanation_quality: ExplanationQualityScore,
    pub fact_scores: Vec<FactScore>,
    pub relevant_visible_facts: u32,
    pub relevant_acquired_facts: u32,
    pub distractor_visible_facts: u32,
    pub distractor_acquired_facts: u32,
    pub total_turns: u32,
}

pub fn score_turn_ledger_deterministically(
    ledger: &TurnLedger,
    evidence_spec: &EvidenceSpec,
) -> Result<DeterministicScoreBreakdown, AppError> {
    score_turn_ledger_with_explanation(ledger, evidence_spec, &UnscoredExplanationQuality)
}

pub fn score_turn_ledger_with_explanation(
    ledger: &TurnLedger,
    evidence_spec: &EvidenceSpec,
    explanation_scorer: &dyn ExplanationQualityScorer,
) -> Result<DeterministicScoreBreakdown, AppError> {
    if ledger.entries.is_empty() {
        return Err(AppError::new(
            ErrorCode::SchemaValidationFailed,
            "turn ledger must contain at least one entry before scoring",
            ErrorContext {
                component: "scoring",
                operation: "score_turn_ledger",
            },
        ));
    }

    let mut fact_scores = Vec::new();
    let mut required_visible = 0_u32;
    let mut required_acquired = 0_u32;
    let mut supporting_visible = 0_u32;
    let mut supporting_acquired = 0_u32;
    let mut distractor_visible = 0_u32;
    let mut distractor_acquired = 0_u32;

    for fact in &evidence_spec.required_facts {
        let fact_score = score_fact(ledger, fact, FactRole::Required);
        required_visible += u32::from(fact_score.visible);
        required_acquired += u32::from(fact_score.acquired);
        fact_scores.push(fact_score);
    }

    for fact in &evidence_spec.supporting_facts {
        let fact_score = score_fact(ledger, fact, FactRole::Supporting);
        supporting_visible += u32::from(fact_score.visible);
        supporting_acquired += u32::from(fact_score.acquired);
        fact_scores.push(fact_score);
    }

    for fact in &evidence_spec.distractor_facts {
        let fact_score = score_fact(ledger, fact, FactRole::Distractor);
        distractor_visible += u32::from(fact_score.visible);
        distractor_acquired += u32::from(fact_score.acquired);
        fact_scores.push(fact_score);
    }

    let total_relevant =
        (evidence_spec.required_facts.len() + evidence_spec.supporting_facts.len()) as u32;
    let relevant_visible = required_visible + supporting_visible;
    let relevant_acquired = required_acquired + supporting_acquired;
    let required_count = evidence_spec.required_facts.len() as u32;

    let required_evidence_recall = ratio(required_acquired, required_count);
    let evidence_visibility_score = ratio(relevant_visible, total_relevant);
    let evidence_acquisition_score = ratio(relevant_acquired, total_relevant);

    let total_acquired = relevant_acquired + distractor_acquired;
    let evidence_precision = ratio(relevant_acquired, total_acquired);

    let total_visible = relevant_visible + distractor_visible;
    let irrelevant_material_ratio = ratio(distractor_visible, total_visible);

    let turns_to_readiness = turns_to_readiness(ledger);
    let reread_count = reread_count(ledger);
    let post_readiness_drift_turns = post_readiness_drift_turns(ledger);

    let readiness_speed = readiness_speed(ledger.entries.len() as u32, turns_to_readiness);
    let reread_efficiency = reciprocal_penalty(reread_count);
    let drift_efficiency = if ready_turn_index(ledger).is_some() {
        reciprocal_penalty(post_readiness_drift_turns)
    } else {
        0.0
    };
    let evidence_efficiency_score = average(&[
        evidence_precision,
        1.0 - irrelevant_material_ratio,
        readiness_speed,
        reread_efficiency,
        drift_efficiency,
    ]);

    let placeholder_report = ScoreReport {
        run_id: ledger.run_id.clone(),
        task_id: ledger.task_id.clone(),
        schema_version: SCORE_REPORT_SCHEMA_VERSION,
        evidence_visibility_score,
        evidence_acquisition_score,
        evidence_efficiency_score,
        explanation_quality_score: 0.0,
        metrics: ScoreMetrics {
            required_evidence_recall,
            evidence_precision,
            irrelevant_material_ratio,
            turns_to_readiness,
            reread_count,
            post_readiness_drift_turns,
        },
    };

    let mut breakdown = DeterministicScoreBreakdown {
        report: placeholder_report,
        explanation_quality: ExplanationQualityScore {
            score: 0.0,
            rationale: String::new(),
        },
        fact_scores,
        relevant_visible_facts: relevant_visible,
        relevant_acquired_facts: relevant_acquired,
        distractor_visible_facts: distractor_visible,
        distractor_acquired_facts: distractor_acquired,
        total_turns: ledger.entries.len() as u32,
    };

    let explanation_quality = explanation_scorer.score(&ExplanationQualityInput {
        ledger,
        evidence_spec,
        deterministic: &breakdown,
    })?;
    validate_normalized_score(explanation_quality.score, "explanation_quality_score")?;
    breakdown.report.explanation_quality_score = explanation_quality.score;
    breakdown.explanation_quality = explanation_quality;
    breakdown.report.validate_for_scoring()?;

    Ok(breakdown)
}

pub fn judge_synthesis(
    ledger: &TurnLedger,
    evidence_spec: &EvidenceSpec,
    deterministic: &DeterministicScoreBreakdown,
    judge: &dyn JudgeAssistedSynthesisScorer,
) -> Result<JudgeAssistedSynthesisScore, AppError> {
    let judged = judge.score(&JudgeAssistedSynthesisInput {
        ledger,
        evidence_spec,
        deterministic,
        explanation_quality: &deterministic.explanation_quality,
    })?;
    validate_normalized_score(judged.score, "judge_assisted_synthesis_score")?;
    if let Some(value) = judged.readiness_correctness {
        validate_normalized_score(value, "judge_assisted_synthesis.readiness_correctness")?;
    }
    if let Some(value) = judged.unsupported_claim_ratio {
        validate_normalized_score(value, "judge_assisted_synthesis.unsupported_claim_ratio")?;
    }
    Ok(judged)
}

fn score_fact(ledger: &TurnLedger, fact: &EvidenceFact, role: FactRole) -> FactScore {
    let mut first_visible_turn = None;
    let mut first_acquired_turn = None;
    let mut observations = Vec::new();

    for entry in &ledger.entries {
        let visible_sources = visible_sources(entry);
        let acquired_sources = acquired_sources(&entry.tool_traces);

        for proof in &fact.acceptable_proofs {
            if let Some(detail) = matches_any_source(proof, &visible_sources) {
                first_visible_turn.get_or_insert(entry.turn_trace.turn_index);
                observations.push(ProofObservation {
                    turn_index: entry.turn_trace.turn_index,
                    proof_kind: proof.kind.clone(),
                    proof_value: proof.value.clone(),
                    channel: EvidenceChannel::VisibleContext,
                    detail,
                });
            }

            if let Some(detail) = matches_any_source(proof, &acquired_sources) {
                first_acquired_turn.get_or_insert(entry.turn_trace.turn_index);
                observations.push(ProofObservation {
                    turn_index: entry.turn_trace.turn_index,
                    proof_kind: proof.kind.clone(),
                    proof_value: proof.value.clone(),
                    channel: EvidenceChannel::ToolTrace,
                    detail,
                });
            }
        }
    }

    observations.sort_by(|left, right| {
        left.turn_index
            .cmp(&right.turn_index)
            .then_with(|| left.detail.cmp(&right.detail))
    });
    observations.dedup_by(|left, right| {
        left.turn_index == right.turn_index
            && left.proof_kind == right.proof_kind
            && left.proof_value == right.proof_value
            && left.channel == right.channel
            && left.detail == right.detail
    });

    FactScore {
        fact_id: fact.fact_id.clone(),
        role,
        visible: first_visible_turn.is_some(),
        acquired: first_acquired_turn.is_some(),
        first_visible_turn,
        first_acquired_turn,
        observations,
    }
}

fn visible_sources(entry: &crate::turn_ledger::TurnLedgerEntry) -> Vec<NamedText> {
    entry
        .turn_trace
        .selection
        .rendered_sections
        .iter()
        .map(|section| NamedText {
            detail: format!("section:{}", section.section_id),
            text: section.content.clone(),
        })
        .collect()
}

fn acquired_sources(tool_traces: &[ToolCallTrace]) -> Vec<NamedText> {
    let mut sources = Vec::new();

    for trace in tool_traces {
        sources.push(NamedText {
            detail: format!("tool:{}", trace.tool_name),
            text: trace.tool_name.clone(),
        });
        sources.push(NamedText {
            detail: format!("tool_input:{}", trace.tool_name),
            text: trace.input_payload.to_string(),
        });
        sources.push(NamedText {
            detail: format!("tool_output:{}", trace.tool_name),
            text: trace.output_payload.to_string(),
        });

        for text in json_strings(&trace.input_payload) {
            sources.push(NamedText {
                detail: format!("tool_input_string:{}", trace.tool_name),
                text,
            });
        }
        for text in json_strings(&trace.output_payload) {
            sources.push(NamedText {
                detail: format!("tool_output_string:{}", trace.tool_name),
                text,
            });
        }
    }

    sources
}

fn matches_any_source(proof: &AcceptableProof, sources: &[NamedText]) -> Option<String> {
    sources
        .iter()
        .find(|source| proof_matches_text(proof, &source.text))
        .map(|source| source.detail.clone())
}

fn proof_matches_text(proof: &AcceptableProof, text: &str) -> bool {
    let needle = normalize_for_match(&proof.value);
    if needle.is_empty() {
        return false;
    }

    let haystack = normalize_for_match(text);
    match proof.kind {
        ProofKind::Excerpt => haystack.contains(&needle),
        ProofKind::Path
        | ProofKind::Symbol
        | ProofKind::LogicalKey
        | ProofKind::GraphPath
        | ProofKind::Coderef => haystack.contains(&needle),
    }
}

fn normalize_for_match(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn json_strings(value: &Value) -> Vec<String> {
    let mut strings = Vec::new();
    collect_json_strings(value, &mut strings);
    strings
}

fn collect_json_strings(value: &Value, strings: &mut Vec<String>) {
    match value {
        Value::String(text) => strings.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_json_strings(item, strings);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                strings.push(key.clone());
                collect_json_strings(item, strings);
            }
        }
        Value::Bool(flag) => strings.push(flag.to_string()),
        Value::Number(number) => strings.push(number.to_string()),
        Value::Null => {}
    }
}

fn turns_to_readiness(ledger: &TurnLedger) -> u32 {
    ready_turn_index(ledger)
        .map(|turn_index| turn_index + 1)
        .unwrap_or(ledger.entries.len() as u32)
}

fn ready_turn_index(ledger: &TurnLedger) -> Option<u32> {
    ledger
        .entries
        .iter()
        .find(|entry| entry.turn_trace.readiness_state == ReadinessState::ReadyToEdit)
        .map(|entry| entry.turn_trace.turn_index)
}

fn reread_count(ledger: &TurnLedger) -> u32 {
    let mut seen = BTreeSet::new();
    let mut rereads = 0_u32;

    for entry in &ledger.entries {
        for trace in &entry.tool_traces {
            if !is_read_like_tool(&trace.tool_name) {
                continue;
            }

            if let Some(target) = trace_target(trace) {
                let key = format!("{}::{target}", normalize_tool_name(&trace.tool_name));
                if !seen.insert(key) {
                    rereads += 1;
                }
            }
        }
    }

    rereads
}

fn post_readiness_drift_turns(ledger: &TurnLedger) -> u32 {
    let Some(ready_turn) = ready_turn_index(ledger) else {
        return 0;
    };

    ledger
        .entries
        .iter()
        .filter(|entry| entry.turn_trace.turn_index > ready_turn)
        .count() as u32
}

fn is_read_like_tool(tool_name: &str) -> bool {
    let normalized = normalize_tool_name(tool_name);
    matches!(
        normalized,
        "graph.describe" | "graph.resolve" | "session.hydrate" | "session.hydrate_source"
    )
}

fn trace_target(trace: &ToolCallTrace) -> Option<String> {
    trace
        .input_payload
        .get("target")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            trace
                .input_payload
                .get("selector")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            trace
                .output_payload
                .get("target")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn normalize_tool_name(tool_name: &str) -> &str {
    tool_name
        .split_once('@')
        .map_or(tool_name, |(base, _)| base)
}

fn ratio(numerator: u32, denominator: u32) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        f64::from(numerator) / f64::from(denominator)
    }
}

fn reciprocal_penalty(value: u32) -> f64 {
    1.0 / (1.0 + f64::from(value))
}

fn readiness_speed(total_turns: u32, turns_to_readiness: u32) -> f64 {
    if total_turns == 0 || turns_to_readiness == 0 || turns_to_readiness > total_turns {
        0.0
    } else {
        f64::from(total_turns - turns_to_readiness + 1) / f64::from(total_turns)
    }
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn validate_normalized_score(value: f64, field: &'static str) -> Result<(), AppError> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::SchemaValidationFailed,
            format!("{field} must be between 0.0 and 1.0"),
            ErrorContext {
                component: "scoring",
                operation: "validate_normalized_score",
            },
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedText {
    detail: String,
    text: String,
}

#[cfg(test)]
mod tests {
    use super::{
        judge_synthesis, score_turn_ledger_deterministically, score_turn_ledger_with_explanation,
        ExplanationQualityInput, ExplanationQualityScore, ExplanationQualityScorer, FactRole,
        JudgeAssistedSynthesisInput, JudgeAssistedSynthesisScore, JudgeAssistedSynthesisScorer,
        UnscoredExplanationQuality,
    };
    use crate::runtime::RuntimeLoopState;
    use crate::tools::ToolCallTrace;
    use crate::turn_ledger::TurnLedgerEntry;
    use graphbench_core::artifacts::{
        AcceptableProof, EvidenceFact, EvidenceSpec, ProofKind, ReadinessState,
        RenderedContextSection, ScoreMetrics, ScoreReport, TurnHashSet, TurnRequest, TurnResponse,
        TurnSelection, TurnTrace, VerificationTarget, CONTEXT_WINDOW_SECTION_SCHEMA_VERSION,
        SCORE_REPORT_SCHEMA_VERSION, TURN_TRACE_SCHEMA_VERSION,
    };
    use serde_json::{json, Value};

    struct FixedExplanationScore(f64);

    impl ExplanationQualityScorer for FixedExplanationScore {
        fn score(
            &self,
            _input: &ExplanationQualityInput<'_>,
        ) -> Result<ExplanationQualityScore, graphbench_core::AppError> {
            Ok(ExplanationQualityScore {
                score: self.0,
                rationale: "scored by test".to_owned(),
            })
        }
    }

    struct FixedJudgeScore;

    impl JudgeAssistedSynthesisScorer for FixedJudgeScore {
        fn score(
            &self,
            _input: &JudgeAssistedSynthesisInput<'_>,
        ) -> Result<JudgeAssistedSynthesisScore, graphbench_core::AppError> {
            Ok(JudgeAssistedSynthesisScore {
                score: 0.75,
                rationale: "judge layer".to_owned(),
                readiness_correctness: Some(1.0),
                unsupported_claim_ratio: Some(0.25),
            })
        }
    }

    #[test]
    fn scoring_produces_deterministic_report_from_turn_ledger() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();

        let scored = score_turn_ledger_deterministically(&ledger, &evidence)
            .expect("deterministic scoring should succeed");

        assert_eq!(scored.report.schema_version, SCORE_REPORT_SCHEMA_VERSION);
        assert_eq!(scored.report.run_id, "run-1");
        assert_eq!(scored.report.task_id, "task-1");
        assert_eq!(scored.report.evidence_visibility_score, 2.0 / 3.0);
        assert_eq!(scored.report.evidence_acquisition_score, 1.0);
        assert_eq!(scored.report.metrics.required_evidence_recall, 1.0);
        assert_eq!(scored.report.metrics.evidence_precision, 1.0);
        assert_eq!(scored.report.metrics.irrelevant_material_ratio, 1.0 / 3.0);
        assert_eq!(scored.report.metrics.turns_to_readiness, 2);
        assert_eq!(scored.report.metrics.reread_count, 1);
        assert_eq!(scored.report.metrics.post_readiness_drift_turns, 1);
        assert_eq!(
            scored
                .fact_scores
                .iter()
                .filter(|fact| fact.role == FactRole::Required && fact.acquired)
                .count(),
            2
        );
        assert_eq!(scored.report.explanation_quality_score, 0.0);
        scored
            .report
            .validate_for_scoring()
            .expect("report should validate");
    }

    #[test]
    fn explanation_quality_score_is_pluggable() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();

        let scored =
            score_turn_ledger_with_explanation(&ledger, &evidence, &FixedExplanationScore(0.9))
                .expect("scoring with explanation score should succeed");

        assert_eq!(scored.report.explanation_quality_score, 0.9);
        assert_eq!(scored.explanation_quality.rationale, "scored by test");
    }

    #[test]
    fn judge_assisted_synthesis_is_secondary_layer() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();
        let scored =
            score_turn_ledger_with_explanation(&ledger, &evidence, &UnscoredExplanationQuality)
                .expect("scoring should succeed");

        let judged =
            judge_synthesis(&ledger, &evidence, &scored, &FixedJudgeScore).expect("judge layer");

        assert_eq!(judged.score, 0.75);
        assert_eq!(judged.readiness_correctness, Some(1.0));
        assert_eq!(judged.unsupported_claim_ratio, Some(0.25));
    }

    #[test]
    fn invalid_explanation_quality_score_is_rejected() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();

        let error =
            score_turn_ledger_with_explanation(&ledger, &evidence, &FixedExplanationScore(1.5))
                .expect_err("invalid score should fail");

        assert_eq!(
            error.to_string(),
            "GB-SCHEMA-001 [scoring:validate_normalized_score] explanation_quality_score must be between 0.0 and 1.0"
        );
    }

    fn sample_evidence_spec() -> EvidenceSpec {
        EvidenceSpec {
            evidence_spec_id: "evidence-1".to_owned(),
            schema_version: 1,
            required_facts: vec![
                EvidenceFact {
                    fact_id: "fact-visible-file".to_owned(),
                    description: "The required file path is visible and acquired.".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::Path,
                        value: "src/lib.rs".to_owned(),
                    }],
                },
                EvidenceFact {
                    fact_id: "fact-excerpt".to_owned(),
                    description: "The validating excerpt is visible and acquired.".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::Excerpt,
                        value: "critical guard".to_owned(),
                    }],
                },
            ],
            supporting_facts: vec![EvidenceFact {
                fact_id: "supporting-symbol".to_owned(),
                description: "A supporting symbol was also acquired.".to_owned(),
                acceptable_proofs: vec![AcceptableProof {
                    kind: ProofKind::Symbol,
                    value: "crate::engine::Runner".to_owned(),
                }],
            }],
            distractor_facts: vec![EvidenceFact {
                fact_id: "distractor".to_owned(),
                description: "A nearby but irrelevant file appeared.".to_owned(),
                acceptable_proofs: vec![AcceptableProof {
                    kind: ProofKind::Path,
                    value: "src/noise.rs".to_owned(),
                }],
            }],
            verification_targets: vec![VerificationTarget {
                kind: "command".to_owned(),
                value: "cargo test".to_owned(),
            }],
        }
    }

    fn sample_ledger() -> crate::turn_ledger::TurnLedger {
        crate::turn_ledger::TurnLedger {
            run_id: "run-1".to_owned(),
            task_id: "task-1".to_owned(),
            fixture_id: "fixture-1".to_owned(),
            entries: vec![
                entry(
                    0,
                    ReadinessState::EvidenceAcquired,
                    vec![
                        section(
                            "active_code_windows",
                            "src/lib.rs\nfn main() { critical guard }\n",
                        ),
                        section(
                            "code_navigation_items",
                            "- src/noise.rs | kind=file | path=src/noise.rs",
                        ),
                    ],
                    vec![
                        tool_trace(
                            "session.hydrate_source@v1",
                            json!({ "target": "src/lib.rs", "padding": 2 }),
                            json!({
                                "status": "ok",
                                "target": "src/lib.rs",
                                "session_export": {
                                    "focus_label": "crate::engine::Runner",
                                    "snippet": "if critical guard { return; }"
                                }
                            }),
                        ),
                        tool_trace(
                            "graph.describe@v1",
                            json!({ "selector": "crate::engine::Runner" }),
                            json!({ "status": "ok", "selector": "crate::engine::Runner" }),
                        ),
                    ],
                ),
                entry(
                    1,
                    ReadinessState::ReadyToEdit,
                    vec![section(
                        "selected_history",
                        "Confirmed critical guard in src/lib.rs",
                    )],
                    vec![tool_trace(
                        "session.hydrate_source@v1",
                        json!({ "target": "src/lib.rs", "padding": 2 }),
                        json!({
                            "status": "ok",
                            "target": "src/lib.rs",
                            "session_export": {
                                "focus_label": "crate::engine::Runner",
                                "snippet": "if critical guard { return; }"
                            }
                        }),
                    )],
                ),
                entry(
                    2,
                    ReadinessState::ReadyToEdit,
                    vec![section("selected_history", "Extra turn after readiness")],
                    vec![],
                ),
            ],
        }
    }

    fn entry(
        turn_index: u32,
        readiness_state: ReadinessState,
        sections: Vec<RenderedContextSection>,
        tool_traces: Vec<ToolCallTrace>,
    ) -> TurnLedgerEntry {
        TurnLedgerEntry {
            turn_trace: TurnTrace {
                run_id: "run-1".to_owned(),
                turn_index,
                task_id: "task-1".to_owned(),
                fixture_id: "fixture-1".to_owned(),
                strategy_id: "graph.targeted-lexical-read".to_owned(),
                request: TurnRequest {
                    schema_version: TURN_TRACE_SCHEMA_VERSION,
                    prompt_version: "v1".to_owned(),
                    prompt_hash: format!("sha256:{}", "a".repeat(64)),
                    context_hash: format!("sha256:{}", "b".repeat(64)),
                },
                response: TurnResponse {
                    provider: "test".to_owned(),
                    model_slug: "model".to_owned(),
                    schema_version: TURN_TRACE_SCHEMA_VERSION,
                    validated: true,
                },
                selection: TurnSelection {
                    selected_context_objects: vec!["context-1".to_owned()],
                    omitted_candidates: Vec::new(),
                    rendered_sections: sections,
                },
                telemetry: graphbench_core::TelemetryCounts {
                    prompt_bytes: 64,
                    prompt_tokens: 16,
                    latency_ms: 10,
                    tool_calls: tool_traces.len() as u32,
                },
                evidence_delta: Vec::new(),
                readiness_state,
                readiness_reason: "scoring test".to_owned(),
                hashes: TurnHashSet {
                    turn_hash: format!("sha256:{}", "c".repeat(64)),
                },
            },
            state_before: RuntimeLoopState::Think,
            state_after: RuntimeLoopState::Act,
            graph_session_before: "{}".to_owned(),
            graph_session_after: "{}".to_owned(),
            ordered_context_object_ids: vec!["context-1".to_owned()],
            compactions: Vec::new(),
            section_accounting: Vec::new(),
            rendered_prompt: "prompt".to_owned(),
            rendered_context: "context".to_owned(),
            tool_traces,
            replay_hash: "sha256:efc0d0a6e6451bf6f2af5793d98499f72f2df7d3961b7f7fbb0110f0e03f4bf1"
                .to_owned(),
        }
    }

    fn section(section_id: &str, content: &str) -> RenderedContextSection {
        RenderedContextSection {
            section_id: section_id.to_owned(),
            schema_version: CONTEXT_WINDOW_SECTION_SCHEMA_VERSION,
            title: section_id.to_owned(),
            content: content.to_owned(),
            byte_count: content.len() as u32,
            token_count: 8,
        }
    }

    fn tool_trace(tool_name: &str, input_payload: Value, output_payload: Value) -> ToolCallTrace {
        ToolCallTrace {
            tool_name: tool_name.to_owned(),
            latency_ms: 5,
            outcome: "ok".to_owned(),
            input_payload,
            output_payload,
        }
    }

    #[test]
    fn report_shape_matches_summary_contract() {
        let report = ScoreReport {
            run_id: "run-1".to_owned(),
            task_id: "task-1".to_owned(),
            schema_version: SCORE_REPORT_SCHEMA_VERSION,
            evidence_visibility_score: 1.0,
            evidence_acquisition_score: 1.0,
            evidence_efficiency_score: 0.5,
            explanation_quality_score: 0.0,
            metrics: ScoreMetrics {
                required_evidence_recall: 1.0,
                evidence_precision: 0.5,
                irrelevant_material_ratio: 0.5,
                turns_to_readiness: 1,
                reread_count: 0,
                post_readiness_drift_turns: 0,
            },
        };

        report
            .validate_for_scoring()
            .expect("summary report contract");
    }

    #[test]
    fn smoke_deterministic_scoring_computes_all_metrics() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();

        let result = score_turn_ledger_deterministically(&ledger, &evidence)
            .expect("scoring should succeed");

        assert!(result.report.evidence_visibility_score >= 0.0);
        assert!(result.report.evidence_visibility_score <= 1.0);

        assert!(result.report.evidence_acquisition_score >= 0.0);
        assert!(result.report.evidence_acquisition_score <= 1.0);

        assert!(result.report.evidence_efficiency_score >= 0.0);
        assert!(result.report.evidence_efficiency_score <= 1.0);

        assert!(result.report.metrics.required_evidence_recall >= 0.0);
        assert!(result.report.metrics.required_evidence_recall <= 1.0);

        assert!(result.report.metrics.evidence_precision >= 0.0);
        assert!(result.report.metrics.evidence_precision <= 1.0);

        assert!(result.report.metrics.irrelevant_material_ratio >= 0.0);
        assert!(result.report.metrics.irrelevant_material_ratio <= 1.0);

        assert!(result.report.metrics.turns_to_readiness >= 1);

        assert!(result.report.metrics.reread_count >= 1);

        assert!(result.report.metrics.post_readiness_drift_turns >= 1);
    }

    #[test]
    fn smoke_empty_ledger_returns_error() {
        let empty_ledger = crate::turn_ledger::TurnLedger {
            run_id: "run-empty".to_owned(),
            task_id: "task-empty".to_owned(),
            fixture_id: "fixture-1".to_owned(),
            entries: vec![],
        };
        let evidence = sample_evidence_spec();

        let error = score_turn_ledger_deterministically(&empty_ledger, &evidence)
            .expect_err("empty ledger should fail");

        assert!(error
            .to_string()
            .contains("turn ledger must contain at least one entry"));
    }

    #[test]
    fn smoke_fact_scoring_tracks_observations() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();

        let result = score_turn_ledger_deterministically(&ledger, &evidence)
            .expect("scoring should succeed");

        let fact_ids: Vec<_> = result.fact_scores.iter().map(|f| &f.fact_id).collect();
        assert!(fact_ids.contains(&&"fact-visible-file".to_owned()));
        assert!(fact_ids.contains(&&"fact-excerpt".to_owned()));
        assert!(fact_ids.contains(&&"supporting-symbol".to_owned()));
        assert!(fact_ids.contains(&&"distractor".to_owned()));

        let acquired: Vec<_> = result
            .fact_scores
            .iter()
            .filter(|f| f.acquired)
            .map(|f| &f.fact_id)
            .collect();
        assert!(
            !acquired.is_empty(),
            "should have at least one acquired fact"
        );
    }

    #[test]
    fn smoke_explanation_quality_scorer_trait_is_pluggable() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();

        struct CustomScorer;
        impl ExplanationQualityScorer for CustomScorer {
            fn score(
                &self,
                _input: &ExplanationQualityInput<'_>,
            ) -> Result<ExplanationQualityScore, graphbench_core::AppError> {
                Ok(ExplanationQualityScore {
                    score: 0.42,
                    rationale: "custom scorer".to_owned(),
                })
            }
        }

        let result = score_turn_ledger_with_explanation(&ledger, &evidence, &CustomScorer)
            .expect("should work with custom scorer");

        assert_eq!(result.report.explanation_quality_score, 0.42);
    }

    #[test]
    fn smoke_judge_assisted_scorer_trait_works() {
        let ledger = sample_ledger();
        let evidence = sample_evidence_spec();

        struct CustomJudge;
        impl JudgeAssistedSynthesisScorer for CustomJudge {
            fn score(
                &self,
                _input: &JudgeAssistedSynthesisInput<'_>,
            ) -> Result<JudgeAssistedSynthesisScore, graphbench_core::AppError> {
                Ok(JudgeAssistedSynthesisScore {
                    score: 0.85,
                    rationale: "custom judge".to_owned(),
                    readiness_correctness: Some(0.9),
                    unsupported_claim_ratio: Some(0.1),
                })
            }
        }

        let deterministic = score_turn_ledger_deterministically(&ledger, &evidence)
            .expect("deterministic scoring first");

        let judged = judge_synthesis(&ledger, &evidence, &deterministic, &CustomJudge)
            .expect("judge should succeed");

        assert_eq!(judged.score, 0.85);
        assert_eq!(judged.readiness_correctness, Some(0.9));
        assert_eq!(judged.unsupported_claim_ratio, Some(0.1));
    }

    #[test]
    fn smoke_proof_matching_works_for_different_proof_kinds() {
        let evidence = EvidenceSpec {
            evidence_spec_id: "proof-test".to_owned(),
            schema_version: 1,
            required_facts: vec![
                EvidenceFact {
                    fact_id: "path-proof".to_owned(),
                    description: "Path proof".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::Path,
                        value: "src/lib.rs".to_owned(),
                    }],
                },
                EvidenceFact {
                    fact_id: "symbol-proof".to_owned(),
                    description: "Symbol proof".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::Symbol,
                        value: "MyStruct".to_owned(),
                    }],
                },
                EvidenceFact {
                    fact_id: "excerpt-proof".to_owned(),
                    description: "Excerpt proof".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::Excerpt,
                        value: "fn main".to_owned(),
                    }],
                },
                EvidenceFact {
                    fact_id: "logical-key-proof".to_owned(),
                    description: "Logical key proof".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::LogicalKey,
                        value: "config.key".to_owned(),
                    }],
                },
            ],
            supporting_facts: vec![],
            distractor_facts: vec![],
            verification_targets: vec![],
        };

        let ledger = crate::turn_ledger::TurnLedger {
            run_id: "run-proofs".to_owned(),
            task_id: "task-proofs".to_owned(),
            fixture_id: "fixture-1".to_owned(),
            entries: vec![entry(
                0,
                ReadinessState::EvidenceAcquired,
                vec![section(
                    "test",
                    "src/lib.rs contains fn main and MyStruct with config.key",
                )],
                vec![],
            )],
        };

        let result = score_turn_ledger_deterministically(&ledger, &evidence)
            .expect("scoring with multiple proof kinds should succeed");

        assert!(result
            .fact_scores
            .iter()
            .any(|f| f.fact_id == "path-proof" && f.visible));
        assert!(result
            .fact_scores
            .iter()
            .any(|f| f.fact_id == "symbol-proof" && f.visible));
        assert!(result
            .fact_scores
            .iter()
            .any(|f| f.fact_id == "excerpt-proof" && f.visible));
        assert!(result
            .fact_scores
            .iter()
            .any(|f| f.fact_id == "logical-key-proof" && f.visible));
    }

    #[test]
    fn smoke_no_readiness_state_means_max_turns_to_readiness() {
        let evidence = sample_evidence_spec();
        let ledger = crate::turn_ledger::TurnLedger {
            run_id: "run-no-ready".to_owned(),
            task_id: "task-no-ready".to_owned(),
            fixture_id: "fixture-1".to_owned(),
            entries: vec![
                entry(0, ReadinessState::EvidenceAcquired, vec![], vec![]),
                entry(1, ReadinessState::EvidenceAcquired, vec![], vec![]),
                entry(2, ReadinessState::EvidenceAcquired, vec![], vec![]),
            ],
        };

        let result = score_turn_ledger_deterministically(&ledger, &evidence)
            .expect("should score even without readiness");

        assert_eq!(result.report.metrics.turns_to_readiness, 3);
    }

    #[test]
    fn smoke_distractor_facts_impact_irrelevant_material_ratio() {
        let evidence = EvidenceSpec {
            evidence_spec_id: "distractor-test".to_owned(),
            schema_version: 1,
            required_facts: vec![EvidenceFact {
                fact_id: "required".to_owned(),
                description: "Required fact".to_owned(),
                acceptable_proofs: vec![AcceptableProof {
                    kind: ProofKind::Path,
                    value: "src/main.rs".to_owned(),
                }],
            }],
            supporting_facts: vec![],
            distractor_facts: vec![
                EvidenceFact {
                    fact_id: "distractor1".to_owned(),
                    description: "Distractor 1".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::Path,
                        value: "src/noise.rs".to_owned(),
                    }],
                },
                EvidenceFact {
                    fact_id: "distractor2".to_owned(),
                    description: "Distractor 2".to_owned(),
                    acceptable_proofs: vec![AcceptableProof {
                        kind: ProofKind::Path,
                        value: "src/garbage.rs".to_owned(),
                    }],
                },
            ],
            verification_targets: vec![],
        };

        let ledger = crate::turn_ledger::TurnLedger {
            run_id: "run-dist".to_owned(),
            task_id: "task-dist".to_owned(),
            fixture_id: "fixture-1".to_owned(),
            entries: vec![entry(
                0,
                ReadinessState::ReadyToEdit,
                vec![
                    section("test", "src/main.rs is the file"),
                    section("test2", "src/noise.rs is noise"),
                    section("test3", "src/garbage.rs is garbage"),
                ],
                vec![],
            )],
        };

        let result = score_turn_ledger_deterministically(&ledger, &evidence)
            .expect("should score with distractors");

        assert!(result.distractor_visible_facts >= 2);
    }

    #[test]
    fn smoke_reread_count_detects_same_file_read_twice() {
        let evidence = sample_evidence_spec();
        let ledger = crate::turn_ledger::TurnLedger {
            run_id: "run-reread".to_owned(),
            task_id: "task-reread".to_owned(),
            fixture_id: "fixture-1".to_owned(),
            entries: vec![
                entry(
                    0,
                    ReadinessState::EvidenceAcquired,
                    vec![],
                    vec![tool_trace(
                        "session.hydrate_source@v1",
                        json!({ "target": "src/lib.rs" }),
                        json!({ "status": "ok" }),
                    )],
                ),
                entry(
                    1,
                    ReadinessState::EvidenceAcquired,
                    vec![],
                    vec![tool_trace(
                        "session.hydrate_source@v1",
                        json!({ "target": "src/lib.rs" }),
                        json!({ "status": "ok" }),
                    )],
                ),
            ],
        };

        let result =
            score_turn_ledger_deterministically(&ledger, &evidence).expect("should detect rereads");

        assert!(result.report.metrics.reread_count >= 1);
    }
}
