use graphbench_core::artifacts::{
    ReadinessState, RenderedContextSection, TelemetryCounts, TurnHashSet, TurnRequest,
    TurnResponse, TurnSelection, TurnTrace,
};
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use graphbench_core::graph::GraphPromptHooks;
use graphbench_core::{
    ContextWindowCompactionPolicy, GraphDiscoveryMode, ProjectionMode, RereadMode,
    SectionTrimDirection, StrategyConfig,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::str::FromStr;
use std::time::Instant;
use ucm_core::BlockId;
use ucp_llm::IdMapper;

use crate::tools::ToolRegistry;
use crate::turn_ledger::{
    CompactionRecord, LedgerSectionAccounting, RuntimeStreamItem, TurnLedger, TurnLedgerEntry,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessRunConfig {
    pub turn_budget: u32,
    pub timeout_ms: u64,
    pub token_budget: u32,
    pub prompt_headroom: u32,
    pub prompt_version: String,
    pub strategy: StrategyConfig,
    pub harness_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectiveState {
    pub task_statement: String,
    pub task_class: String,
    pub allowed_tools: Vec<String>,
    pub verification_targets: Vec<String>,
    pub unresolved_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessInput {
    pub run_id: String,
    pub fixture_id: String,
    pub task_id: String,
    pub objective: ObjectiveState,
    pub graph_prompt: GraphPromptHooks,
    pub graph_session_snapshot: String,
    pub config: HarnessRunConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelResponseKind {
    Think,
    ToolCall,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessModelResponse {
    pub kind: ModelResponseKind,
    #[serde(default, alias = "_version")]
    pub prompt_version: String,
    #[serde(default)]
    pub model_slug: String,
    #[serde(default)]
    pub provider: String,
    pub assistant_message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<ToolCall>,
    #[serde(default)]
    pub acquired_fact_ids: Vec<String>,
    pub readiness_state: ReadinessState,
}

impl HarnessModelResponse {
    pub fn validate(&self, expected_prompt_version: &str) -> Result<(), AppError> {
        if self.prompt_version != expected_prompt_version {
            return Err(AppError::new(
                ErrorCode::ProviderResponseInvalid,
                "response prompt_version does not match the current harness prompt version",
                ErrorContext {
                    component: "harness",
                    operation: "validate_model_response",
                },
            ));
        }

        if self.model_slug.trim().is_empty() || self.provider.trim().is_empty() {
            return Err(AppError::new(
                ErrorCode::ProviderResponseInvalid,
                "response model_slug and provider are required",
                ErrorContext {
                    component: "harness",
                    operation: "validate_model_response",
                },
            ));
        }

        match self.kind {
            ModelResponseKind::ToolCall if self.tool_call.is_none() => Err(AppError::new(
                ErrorCode::ProviderResponseInvalid,
                "tool_call responses must carry a tool payload",
                ErrorContext {
                    component: "harness",
                    operation: "validate_model_response",
                },
            )),
            ModelResponseKind::Think | ModelResponseKind::Complete if self.tool_call.is_some() => {
                Err(AppError::new(
                    ErrorCode::ProviderResponseInvalid,
                    "non-tool responses must not carry a tool payload",
                    ErrorContext {
                        component: "harness",
                        operation: "validate_model_response",
                    },
                ))
            }
            _ => Ok(()),
        }
    }
}

pub trait ModelClient {
    fn respond(&mut self, prompt: &str) -> Result<ModelInvocation, AppError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInvocation {
    pub response: HarnessModelResponse,
    pub raw_request: Value,
    pub raw_response: Value,
    pub provider_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HarnessEvent {
    RunStarted {
        run_id: String,
    },
    TurnStarted {
        turn_index: u32,
        state: RuntimeLoopState,
    },
    TurnCompleted {
        turn_index: u32,
    },
    PromptAssembled {
        turn_index: u32,
        prompt_hash: String,
        context_hash: String,
    },
    ModelResponseRejected {
        turn_index: u32,
        error: String,
    },
    ModelResponseValidated {
        turn_index: u32,
        kind: ModelResponseKind,
    },
    ToolStarted {
        turn_index: u32,
        tool_name: String,
    },
    ToolCompleted {
        turn_index: u32,
        tool_name: String,
    },
    ToolFailed {
        turn_index: u32,
        tool_name: String,
        error: String,
    },
    ToolRequested {
        turn_index: u32,
        tool_name: String,
    },
    ModelRequestSent {
        turn_index: u32,
    },
    ModelResponseReceived {
        turn_index: u32,
        provider_request_id: Option<String>,
    },
    ReadinessChanged {
        turn_index: u32,
        readiness_state: ReadinessState,
    },
    GraphSessionMutated {
        mutation: String,
        graph_session_hash: String,
    },
    EvidenceMatched {
        turn_index: u32,
        fact_ids: Vec<String>,
    },
    RunFailed {
        run_id: String,
        error: String,
    },
    RunCompleted {
        run_id: String,
        turns: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessOutput {
    pub turn_ledger: TurnLedger,
    pub model_invocations: Vec<ModelInvocation>,
    pub final_state: RuntimeLoopState,
    pub final_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLoopState {
    Init,
    Think,
    Act,
    Done,
}

pub struct HarnessRunner<'a, M: ModelClient> {
    model: &'a mut M,
    tools: &'a ToolRegistry,
    telemetry_hook: Option<Box<dyn Fn(&HarnessEvent) + 'a>>,
    graph_state_provider: Option<Box<dyn Fn() -> Result<RefreshedGraphState, AppError> + 'a>>,
}

#[derive(Debug, Clone)]
pub struct RefreshedGraphState {
    pub graph_prompt: GraphPromptHooks,
    pub graph_session_snapshot: String,
}

impl<'a, M: ModelClient> HarnessRunner<'a, M> {
    pub fn new(model: &'a mut M, tools: &'a ToolRegistry) -> Self {
        Self {
            model,
            tools,
            telemetry_hook: None,
            graph_state_provider: None,
        }
    }

    pub fn with_telemetry_hook(mut self, hook: impl Fn(&HarnessEvent) + 'a) -> Self {
        self.telemetry_hook = Some(Box::new(hook));
        self
    }

    pub fn with_graph_state_provider(
        mut self,
        provider: impl Fn() -> Result<RefreshedGraphState, AppError> + 'a,
    ) -> Self {
        self.graph_state_provider = Some(Box::new(provider));
        self
    }

    pub fn execute(&mut self, input: &HarnessInput) -> Result<HarnessOutput, AppError> {
        let result = self.execute_inner(input);
        if let Err(error) = &result {
            self.emit(HarnessEvent::RunFailed {
                run_id: input.run_id.clone(),
                error: error.to_string(),
            });
        }
        result
    }

    fn execute_inner(&mut self, input: &HarnessInput) -> Result<HarnessOutput, AppError> {
        input.config.strategy.validate()?;
        let mut state = RuntimeLoopState::Init;
        let mut stream = vec![RuntimeStreamItem::objective(
            input.objective.task_statement.clone(),
        )];
        let mut ledger = TurnLedger::new(
            input.run_id.clone(),
            input.task_id.clone(),
            input.fixture_id.clone(),
        );
        let mut current_graph_prompt = input.graph_prompt.clone();
        let mut current_graph_session_snapshot = input.graph_session_snapshot.clone();
        let mut model_invocations = Vec::new();
        let mut final_message = String::new();
        let run_started = Instant::now();
        self.emit(HarnessEvent::RunStarted {
            run_id: input.run_id.clone(),
        });

        for turn_index in 0..input.config.turn_budget {
            if run_started.elapsed().as_millis() as u64 > input.config.timeout_ms {
                return Err(AppError::new(
                    ErrorCode::ConfigurationInvalid,
                    "harness timeout budget exceeded",
                    ErrorContext {
                        component: "harness",
                        operation: "execute",
                    },
                ));
            }
            let state_before = state;
            state = match state {
                RuntimeLoopState::Init => RuntimeLoopState::Think,
                RuntimeLoopState::Think => RuntimeLoopState::Act,
                RuntimeLoopState::Act => RuntimeLoopState::Think,
                RuntimeLoopState::Done => {
                    return Err(AppError::new(
                        ErrorCode::ConfigurationInvalid,
                        "harness entered an invalid done -> next turn transition",
                        ErrorContext {
                            component: "harness",
                            operation: "execute",
                        },
                    ));
                }
            };
            self.emit(HarnessEvent::TurnStarted { turn_index, state });

            let graph_prompt_before_turn = current_graph_prompt.clone();
            let graph_session_before_turn = current_graph_session_snapshot.clone();
            let (history_items, mut compactions) =
                compact_history(&stream, &input.config.strategy.context_window.compaction);
            let (
                mut sections,
                mut prompt,
                mut rendered_context,
                mut prompt_hash,
                mut context_hash,
                mut token_estimate,
            ) = assemble_prompt(
                input,
                &graph_prompt_before_turn,
                &history_items,
                &self.tools.contracts(),
            );
            if token_estimate > input.config.token_budget {
                let emergency_history = vec![RuntimeStreamItem::summary(summarize_history(
                    &stream,
                    input
                        .config
                        .strategy
                        .context_window
                        .compaction
                        .emergency_summary_max_chars as usize,
                ))];
                let source_item_ids = stream.iter().map(|item| item.item_id.clone()).collect();
                (
                    sections,
                    prompt,
                    rendered_context,
                    prompt_hash,
                    context_hash,
                    token_estimate,
                ) = assemble_prompt(
                    input,
                    &graph_prompt_before_turn,
                    &emergency_history,
                    &self.tools.contracts(),
                );
                compactions = vec![CompactionRecord {
                    summary_item_id: emergency_history[0].item_id.clone(),
                    source_item_ids,
                }];
            }
            self.emit(HarnessEvent::PromptAssembled {
                turn_index,
                prompt_hash: prompt_hash.clone(),
                context_hash: context_hash.clone(),
            });

            let started = Instant::now();
            self.emit(HarnessEvent::ModelRequestSent { turn_index });
            let invocation = self.model.respond(&prompt)?;
            self.emit(HarnessEvent::ModelResponseReceived {
                turn_index,
                provider_request_id: invocation.provider_request_id.clone(),
            });
            let response = invocation.response.clone();
            if let Err(error) = response.validate(&input.config.prompt_version) {
                self.emit(HarnessEvent::ModelResponseRejected {
                    turn_index,
                    error: error.to_string(),
                });
                return Err(error);
            }
            self.emit(HarnessEvent::ModelResponseValidated {
                turn_index,
                kind: response.kind.clone(),
            });
            let mut tool_traces = Vec::new();
            model_invocations.push(invocation);

            stream.push(RuntimeStreamItem::assistant(
                response.assistant_message.clone(),
            ));
            final_message = response.assistant_message.clone();

            if let Some(tool_call) = &response.tool_call {
                let canonical_tool_name = self
                    .tools
                    .canonical_tool_name(&tool_call.tool_name)
                    .unwrap_or_else(|| tool_call.tool_name.clone());
                self.emit(HarnessEvent::ToolRequested {
                    turn_index,
                    tool_name: canonical_tool_name.clone(),
                });
                self.emit(HarnessEvent::ToolStarted {
                    turn_index,
                    tool_name: canonical_tool_name.clone(),
                });
                let result = match self.tools.invoke(tool_call) {
                    Ok(result) => result,
                    Err(error) => {
                        self.emit(HarnessEvent::ToolFailed {
                            turn_index,
                            tool_name: canonical_tool_name.clone(),
                            error: error.to_string(),
                        });
                        // Instead of failing the run, emit a tool error result and continue
                        // This allows the model to try a different tool on the next turn
                        stream.push(RuntimeStreamItem::tool_call(
                            canonical_tool_name.clone(),
                            tool_call.payload.clone(),
                        ));
                        stream.push(RuntimeStreamItem::tool_result(
                            canonical_tool_name.clone(),
                            serde_json::Value::String(format!("Tool invocation failed: {}", error)),
                        ));
                        // Continue to next iteration of the loop instead of returning error
                        continue;
                    }
                };
                tool_traces.push(result.trace.clone());
                self.emit(HarnessEvent::ToolCompleted {
                    turn_index,
                    tool_name: result.canonical_tool_name.clone(),
                });
                stream.push(RuntimeStreamItem::tool_call(
                    result.canonical_tool_name.clone(),
                    tool_call.payload.clone(),
                ));
                stream.push(RuntimeStreamItem::tool_result(
                    result.canonical_tool_name.clone(),
                    result.output.clone(),
                ));
                if let (Some(summary), Some(provider)) = (
                    result.mutation_summary.as_deref(),
                    &self.graph_state_provider,
                ) {
                    let refreshed = provider()?;
                    current_graph_prompt = refreshed.graph_prompt;
                    current_graph_session_snapshot = refreshed.graph_session_snapshot;
                    self.emit(HarnessEvent::GraphSessionMutated {
                        mutation: summary.to_owned(),
                        graph_session_hash: sha256_string(&current_graph_session_snapshot),
                    });
                }
            }

            if response.kind == ModelResponseKind::Complete {
                state = RuntimeLoopState::Done;
            }
            self.emit(HarnessEvent::ReadinessChanged {
                turn_index,
                readiness_state: response.readiness_state.clone(),
            });
            if !response.acquired_fact_ids.is_empty() {
                self.emit(HarnessEvent::EvidenceMatched {
                    turn_index,
                    fact_ids: response.acquired_fact_ids.clone(),
                });
            }

            let selection = TurnSelection {
                selected_context_objects: graph_prompt_before_turn
                    .context_objects
                    .iter()
                    .map(|object| object.context_object_id.clone())
                    .collect(),
                omitted_candidates: graph_prompt_before_turn.omitted_candidates.clone(),
                rendered_sections: sections.clone(),
            };
            let telemetry = TelemetryCounts {
                prompt_bytes: prompt.len() as u32,
                prompt_tokens: token_estimate,
                latency_ms: started.elapsed().as_millis() as u32,
                tool_calls: tool_traces.len() as u32,
            };
            let trace = TurnTrace {
                run_id: input.run_id.clone(),
                turn_index,
                task_id: input.task_id.clone(),
                fixture_id: input.fixture_id.clone(),
                strategy_id: input.config.strategy.strategy_id.clone(),
                request: TurnRequest {
                    schema_version: graphbench_core::artifacts::TURN_TRACE_SCHEMA_VERSION,
                    prompt_version: input.config.prompt_version.clone(),
                    prompt_hash: prompt_hash.clone(),
                    context_hash: context_hash.clone(),
                },
                response: TurnResponse {
                    provider: response.provider.clone(),
                    model_slug: response.model_slug.clone(),
                    schema_version: graphbench_core::artifacts::TURN_TRACE_SCHEMA_VERSION,
                    validated: true,
                },
                selection,
                telemetry,
                evidence_delta: response.acquired_fact_ids.clone(),
                readiness_state: response.readiness_state,
                readiness_reason: response.assistant_message.clone(),
                hashes: TurnHashSet {
                    turn_hash: sha256_string(&format!("{turn_index}:{prompt_hash}:{context_hash}")),
                },
            };
            trace.validate_for_creation()?;
            trace.validate_for_persistence()?;
            trace.validate_for_replay()?;

            let entry = TurnLedgerEntry {
                turn_trace: trace,
                state_before,
                state_after: state,
                graph_session_before: graph_session_before_turn,
                graph_session_after: current_graph_session_snapshot.clone(),
                ordered_context_object_ids: graph_prompt_before_turn
                    .context_objects
                    .iter()
                    .map(|object| object.context_object_id.clone())
                    .collect(),
                compactions,
                section_accounting: sections
                    .iter()
                    .map(|section| LedgerSectionAccounting {
                        section_id: section.section_id.clone(),
                        byte_count: section.byte_count,
                        token_count: section.token_count,
                    })
                    .collect(),
                rendered_prompt: prompt,
                rendered_context,
                tool_traces,
                replay_hash: sha256_string(&render_prompt(&sections)),
            };
            ledger.push(entry)?;

            self.emit(HarnessEvent::TurnCompleted { turn_index });

            if state == RuntimeLoopState::Done {
                break;
            }
        }

        if state != RuntimeLoopState::Done {
            return Err(AppError::new(
                ErrorCode::ConfigurationInvalid,
                "harness turn budget exhausted before completion",
                ErrorContext {
                    component: "harness",
                    operation: "execute",
                },
            ));
        }

        Ok(HarnessOutput {
            turn_ledger: ledger,
            model_invocations,
            final_state: state,
            final_message,
        })
        .inspect(|output| {
            self.emit(HarnessEvent::RunCompleted {
                run_id: input.run_id.clone(),
                turns: output.turn_ledger.entries.len(),
            });
        })
    }

    fn emit(&self, event: HarnessEvent) {
        if let Some(hook) = &self.telemetry_hook {
            hook(&event);
        }
    }
}

fn canonical_sections(
    input: &HarnessInput,
    graph_prompt: &GraphPromptHooks,
    history_items: &[RuntimeStreamItem],
    tool_contracts: &[crate::tools::ToolContract],
) -> Vec<RenderedContextSection> {
    let strategy = &input.config.strategy;
    let base_runtime = [
        "GraphBench runtime. Stay bounded, typed, and evidence-oriented.",
        "",
        &format!(
            "Strategy: {}@{}",
            strategy.strategy_id, strategy.strategy_version
        ),
        "",
        "## Rules",
        "1. Return JSON only, with no markdown or extra prose outside the response object.",
        "2. Use exact short ids as provided in the prompt when referring to graph nodes.",
        "3. Short ids are stable within this turn and look like F1, S3, D2, or R1.",
        "4. Tool calls must use one registered tool name exactly as listed below.",
        "5. Prefer graph and session traversal over blind lexical browsing.",
        "6. Responses must be schema-valid.",
        strategy_graph_directive(strategy),
        strategy_projection_directive(strategy),
        strategy_reread_directive(strategy),
    ]
    .join("\n");
    let selected_history = history_items
        .iter()
        .map(RuntimeStreamItem::render)
        .collect::<Vec<_>>()
        .join("\n");
    let tool_contracts = tool_contracts
        .iter()
        .map(|contract| {
            format!(
                "- {}@{}: {}",
                contract.name, contract.version, contract.input_description
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let sections = vec![
        section(
            "base_runtime_instructions",
            "Base Runtime Instructions",
            &base_runtime,
        ),
        section(
            "response_contract",
            "Response Contract",
            concat!(concat!(
                "Return JSON only with this exact shape: ",
                "{{\"kind\":\"think|tool_call|complete\",",
                "\"assistant_message\":\"<brief status>\",",
                "\"tool_call\":{{\"tool_name\":\"<one registered tool name>\",\"payload\":{{}}}},",
                "\"acquired_fact_ids\":[\"<fact id>\"],",
                "\"readiness_state\":\"not_ready|evidence_visible|evidence_acquired|ready_to_edit\"}}. ",
                "Use null or omit tool_call unless kind is tool_call. ",
                "When kind is tool_call, tool_call.tool_name must exactly match one of the registered tool names shown below. ",
                "Do not invent helper tools, verifier tools, or meta tools."
            )),
        ),
        section(
            "objective_state",
            "Objective State",
            &format!(
                "Task: {}\nClass: {}\nAllowed Tools:\n{}\nVerification Targets:\n{}",
                input.objective.task_statement,
                input.objective.task_class,
                input.objective.allowed_tools.join("\n"),
                input.objective.verification_targets.join("\n")
            ),
        ),
        section("selected_history", "Selected History", &selected_history),
        graph_prompt.active_code_windows.clone(),
        graph_prompt.code_navigation_items.clone(),
        graph_prompt.graph_relations.clone(),
        graph_prompt.graph_frontier.clone(),
        section("tool_contracts", "Tool Contracts", &tool_contracts),
    ];
    apply_section_budgets(&shorten_section_ids(&sections), &input.config.strategy)
}

fn assemble_prompt(
    input: &HarnessInput,
    graph_prompt: &GraphPromptHooks,
    history_items: &[RuntimeStreamItem],
    tool_contracts: &[crate::tools::ToolContract],
) -> (
    Vec<RenderedContextSection>,
    String,
    String,
    String,
    String,
    u32,
) {
    let sections = canonical_sections(input, graph_prompt, history_items, tool_contracts);
    let prompt = render_prompt(&sections);
    let rendered_context = sections
        .iter()
        .map(|section| section.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let prompt_hash = sha256_string(&prompt);
    let context_hash = sha256_string(&rendered_context);
    let token_estimate = approximate_tokens(&prompt);
    (
        sections,
        prompt,
        rendered_context,
        prompt_hash,
        context_hash,
        token_estimate,
    )
}

fn compact_history(
    stream: &[RuntimeStreamItem],
    policy: &ContextWindowCompactionPolicy,
) -> (Vec<RuntimeStreamItem>, Vec<CompactionRecord>) {
    let prepared = if policy.deduplicate_tool_results {
        deduplicate_tool_results(stream)
    } else {
        stream.to_vec()
    };
    let window = policy.history_recent_items as usize;
    if prepared.len() <= window {
        return (prepared, Vec::new());
    }

    let older = &prepared[..prepared.len() - window];
    let summary =
        RuntimeStreamItem::summary(summarize_history(older, policy.summary_max_chars as usize));
    let mut selected = vec![summary.clone()];
    selected.extend_from_slice(&prepared[prepared.len() - window..]);

    (
        selected,
        vec![CompactionRecord {
            summary_item_id: summary.item_id.clone(),
            source_item_ids: older.iter().map(|item| item.item_id.clone()).collect(),
        }],
    )
}

fn summarize_history(items: &[RuntimeStreamItem], max_summary_chars: usize) -> String {
    let joined = items
        .iter()
        .map(RuntimeStreamItem::render_for_summary)
        .collect::<Vec<_>>()
        .join(" | ");
    if joined.len() <= max_summary_chars {
        joined
    } else {
        let mut truncated = joined.chars().take(max_summary_chars).collect::<String>();
        truncated.push_str(" ...[summary truncated]");
        truncated
    }
}

fn deduplicate_tool_results(stream: &[RuntimeStreamItem]) -> Vec<RuntimeStreamItem> {
    let mut seen = HashSet::new();
    let mut selected = Vec::with_capacity(stream.len());
    for item in stream.iter().rev() {
        match &item.payload {
            crate::turn_ledger::RuntimeStreamKind::ToolResult { tool_name, payload } => {
                let key = format!("{tool_name}:{}", payload);
                if seen.insert(key) {
                    selected.push(item.clone());
                }
            }
            _ => selected.push(item.clone()),
        }
    }
    selected.reverse();
    selected
}

fn shorten_section_ids(sections: &[RenderedContextSection]) -> Vec<RenderedContextSection> {
    let block_id_pattern = Regex::new(r"blk_[0-9a-fA-F]{24}").expect("block id regex");
    let mut mapper = IdMapper::new();
    for section in sections {
        for found in block_id_pattern.find_iter(&section.content) {
            if let Ok(block_id) = BlockId::from_str(found.as_str()) {
                mapper.register(&block_id);
            }
        }
    }

    sections
        .iter()
        .map(|section| {
            let shortened = mapper.shorten_text(&section.content);
            RenderedContextSection {
                section_id: section.section_id.clone(),
                schema_version: section.schema_version,
                title: section.title.clone(),
                byte_count: shortened.len() as u32,
                token_count: approximate_tokens(&shortened),
                content: shortened,
            }
        })
        .collect()
}

fn apply_section_budgets(
    sections: &[RenderedContextSection],
    strategy: &StrategyConfig,
) -> Vec<RenderedContextSection> {
    sections
        .iter()
        .map(|section| {
            let Some(budget) = strategy
                .context_window
                .section_budgets
                .iter()
                .find(|budget| budget.section_id == section.section_id)
            else {
                return section.clone();
            };

            let truncated = truncate_text_to_token_budget(
                &section.content,
                budget.max_tokens as usize,
                budget.trim_direction.clone(),
            );
            RenderedContextSection {
                section_id: section.section_id.clone(),
                schema_version: section.schema_version,
                title: section.title.clone(),
                byte_count: truncated.len() as u32,
                token_count: approximate_tokens(&truncated),
                content: truncated,
            }
        })
        .collect()
}

fn truncate_text_to_token_budget(
    content: &str,
    max_tokens: usize,
    direction: SectionTrimDirection,
) -> String {
    let spans = token_spans(content);
    if spans.len() <= max_tokens {
        return content.to_owned();
    }
    let marker = "...[truncated]";
    if max_tokens <= 1 {
        return marker.to_owned();
    }

    let keep = max_tokens - 1;
    match direction {
        SectionTrimDirection::Tail => {
            let end = spans[keep - 1].1;
            let slice = content[..end].trim_end();
            format!("{slice} {marker}")
        }
        SectionTrimDirection::Head => {
            let start = spans[spans.len() - keep].0;
            let slice = content[start..].trim_start();
            format!("{marker} {slice}")
        }
    }
}

fn token_spans(content: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut token_start = None;

    for (index, character) in content.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                spans.push((start, index));
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }

    if let Some(start) = token_start {
        spans.push((start, content.len()));
    }

    spans
}

fn strategy_graph_directive(strategy: &StrategyConfig) -> &'static str {
    match strategy.graph_discovery {
        GraphDiscoveryMode::BroadGraphDiscovery => {
            "7. This strategy favors broad graph frontier expansion before exact lexical reads."
        }
        GraphDiscoveryMode::GraphThenTargetedLexicalRead => {
            "7. This strategy favors graph discovery first, then focused lexical reads only when exact proof is blocking."
        }
    }
}

fn strategy_projection_directive(strategy: &StrategyConfig) -> &'static str {
    match strategy.projection {
        ProjectionMode::Balanced => {
            "8. Use a balanced projection: keep enough structural guidance and enough exact text to stay edit-ready."
        }
        ProjectionMode::HighRecall => {
            "8. Use a high-recall projection: keep broader graph and history context visible before pruning."
        }
        ProjectionMode::Minimal => {
            "8. Use a minimal projection: prefer the smallest sufficient context and trim aggressively."
        }
    }
}

fn strategy_reread_directive(strategy: &StrategyConfig) -> &'static str {
    match strategy.reread_policy {
        RereadMode::Allow => {
            "9. Rereads are allowed, but prefer already visible evidence when possible."
        }
        RereadMode::StrictNoReread => {
            "9. Strict no-reread is active: do not request the same exact lexical read twice if it is already visible."
        }
    }
}

fn section(section_id: &str, title: &str, content: &str) -> RenderedContextSection {
    RenderedContextSection {
        section_id: section_id.to_owned(),
        schema_version: graphbench_core::artifacts::CONTEXT_WINDOW_SECTION_SCHEMA_VERSION,
        title: title.to_owned(),
        content: content.to_owned(),
        byte_count: content.len() as u32,
        token_count: approximate_tokens(content),
    }
}

fn render_prompt(sections: &[RenderedContextSection]) -> String {
    sections
        .iter()
        .map(|section| format!("## {}\n{}", section.title, section.content))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn approximate_tokens(content: &str) -> u32 {
    content.split_whitespace().count() as u32
}

fn sha256_string(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!("sha256:{digest:x}")
}
