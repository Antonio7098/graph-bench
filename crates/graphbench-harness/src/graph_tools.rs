use crate::runtime::RefreshedGraphState;
use crate::tools::{ToolContract, ToolExecutionResult, ToolRegistry};
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use graphbench_core::{GraphSession, GraphWorkspace, RepresentationLevel};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use ucp_codegraph::{
    CodeGraphExpandMode, CodeGraphFindQuery, CodeGraphOperationBudget, CodeGraphTraversalConfig,
};

const UCP_PYTHON_ROOT: &str =
    "/home/antonio/programming/Hivemind/unified-content-protocol/crates/ucp-python";
const UCP_PYTHON_VENV: &str = "target/ucp-python-venv";
const UCP_PYTHON_SCRIPT: &str = r#"
import json
import sys
import ucp

payload = json.load(open(sys.argv[1], 'r', encoding='utf-8'))
raw = ucp.CodeGraph.load(payload['graph_snapshot_path'])
session = raw.load_session_json(payload['session_json'])
args = payload['arguments']

code = args.get('code')
if not code:
    raise ValueError("Missing required 'code' field in arguments")

bindings = args.get('bindings')
include_export = args.get('include_export', False)
export_kwargs = args.get('export_kwargs') or {}
limits = args.get('limits')

result = ucp.run_python_query(
    raw,
    code,
    session=session,
    bindings=bindings,
    include_export=include_export,
    export_kwargs=export_kwargs,
    limits=limits,
)

session = result.session.raw
json.dump(
    {
        'payload': result.as_dict(),
        'session_json': session.to_json(),
    },
    sys.stdout,
)
"#;

#[derive(Clone)]
pub struct LiveGraphState {
    inner: Arc<Mutex<LiveGraphStateInner>>,
}

struct LiveGraphStateInner {
    workspace: GraphWorkspace,
    session: GraphSession,
    graph_snapshot_path: PathBuf,
    render_max_tokens: usize,
}

impl LiveGraphState {
    pub fn new(
        workspace: GraphWorkspace,
        session: GraphSession,
        graph_snapshot_path: PathBuf,
        render_max_tokens: usize,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(LiveGraphStateInner {
                workspace,
                session,
                graph_snapshot_path,
                render_max_tokens,
            })),
        }
    }

    pub fn snapshot(&self) -> Result<RefreshedGraphState, AppError> {
        let inner = self.lock()?;
        Ok(RefreshedGraphState {
            graph_prompt: inner.session.render_for_harness(inner.render_max_tokens),
            graph_session_snapshot: inner.session.session_json()?,
        })
    }

    pub fn register_tools(&self, registry: &mut ToolRegistry) {
        register_graph_describe_tool(registry, self.clone());
        register_graph_resolve_tool(registry, self.clone());
        register_graph_find_tool(registry, self.clone());
        register_graph_path_tool(registry, self.clone());
        register_session_add_tool(registry, self.clone());
        register_session_walk_tool(registry, self.clone());
        register_session_walk_alias_tool(
            registry,
            self.clone(),
            "session.expand_file",
            "file",
            "Payload with target selector and optional depth, limit, relation filters, priority threshold, and traversal budgets.",
        );
        register_session_walk_alias_tool(
            registry,
            self.clone(),
            "session.expand_dependencies",
            "dependencies",
            "Payload with target selector and optional depth, limit, relation filters, priority threshold, and traversal budgets.",
        );
        register_session_walk_alias_tool(
            registry,
            self.clone(),
            "session.expand_dependents",
            "dependents",
            "Payload with target selector and optional depth, limit, relation filters, priority threshold, and traversal budgets.",
        );
        register_session_focus_tool(registry, self.clone());
        register_session_collapse_tool(registry, self.clone());
        register_session_pin_tool(registry, self.clone());
        register_session_prune_tool(registry, self.clone());
        register_session_why_tool(registry, self.clone());
        register_session_explain_export_omission_tool(registry, self.clone());
        register_session_why_pruned_tool(registry, self.clone());
        register_session_export_tool(registry, self.clone());
        register_session_hydrate_tool(registry, self.clone());
        register_session_hydrate_alias_tool(registry, self.clone());
        register_session_recommendations_tool(registry, self.clone());
        register_session_estimate_expand_tool(registry, self.clone());
        register_session_estimate_hydrate_tool(registry, self.clone());
        register_session_mutation_log_tool(registry, self.clone());
        register_session_event_log_tool(registry, self.clone());
        register_run_python_query_tool(registry, self.clone());
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, LiveGraphStateInner>, AppError> {
        self.inner.lock().map_err(|_| {
            AppError::new(
                ErrorCode::ConfigurationInvalid,
                "live graph state lock poisoned",
                ErrorContext {
                    component: "graph_tools",
                    operation: "lock",
                },
            )
        })
    }
}

pub fn ensure_python_query_runtime_ready() -> Result<(), AppError> {
    ensure_ucp_python_runtime().map(|_| ())
}

fn register_graph_describe_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "graph.describe".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with selector string".to_owned(),
            output_description: "Selector resolution details and matching node summary".to_owned(),
        },
        has_string("selector"),
        is_object,
        move |payload| {
            let selector = normalize_selector(payload["selector"].as_str().unwrap_or_default());
            let inner = state.lock()?;
            let resolution = inner.session.explain_selector_json(&selector)?;
            Ok(json!({
                "status": "ok",
                "selector": selector,
                "resolution": resolution,
            }))
        },
    );
}

fn register_graph_resolve_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "graph.resolve".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with selector string".to_owned(),
            output_description: "Selector resolution explanation and resolved block id".to_owned(),
        },
        has_string("selector"),
        is_object,
        move |payload| {
            let selector = normalize_selector(payload["selector"].as_str().unwrap_or_default());
            let inner = state.lock()?;
            let resolution = inner.session.explain_selector_json(&selector)?;
            Ok(json!({
                "status": "ok",
                "selector": selector,
                "resolution": resolution,
            }))
        },
    );
}

fn register_graph_find_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "graph.find".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with optional node_class, name_regex, path_regex, logical_key_regex, exported, case_sensitive, and limit".to_owned(),
            output_description: "Matching graph nodes".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let query = CodeGraphFindQuery {
                node_class: string_field(payload, "node_class"),
                name_regex: string_field(payload, "name_regex"),
                path_regex: string_field(payload, "path_regex"),
                logical_key_regex: string_field(payload, "logical_key_regex"),
                case_sensitive: bool_field(payload, "case_sensitive").unwrap_or(false),
                exported: payload.get("exported").and_then(|value| value.as_bool()),
                limit: usize_field(payload, "limit"),
            };
            let inner = state.lock()?;
            let matches = inner.session.find_nodes_json(query)?;
            Ok(json!({
                "status": "ok",
                "matches": matches,
            }))
        },
    );
}

fn register_graph_path_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "graph.path".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with start selector, end selector, and optional max_hops"
                .to_owned(),
            output_description: "Shortest discovered graph path between two selectors".to_owned(),
        },
        |payload| has_string("start")(payload) && has_string("end")(payload),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let path = inner.session.path_between_json(
                payload["start"].as_str().unwrap_or_default(),
                payload["end"].as_str().unwrap_or_default(),
                usize_field(payload, "max_hops").unwrap_or(8),
            )?;
            Ok(json!({
                "status": "ok",
                "path": path,
            }))
        },
    );
}

fn register_session_add_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.add".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with target selector and optional detail of skeleton|summary|neighborhood|source".to_owned(),
            output_description: "Updated session export after selecting a node at the chosen detail level".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let detail = parse_detail_level(string_field(payload, "detail").as_deref())?;
            let detail_name = detail_label(&detail).to_owned();
            let target = resolved_target_selector(payload, &inner.session)?;
            let before = inner.session.session_json()?;
            inner.session.select(&target, detail)?;
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.add",
                    &target,
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after)
                    .then(|| format!("session.add(target={target},detail={detail_name})")),
            })
        },
    );
}

fn register_session_walk_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.walk".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with target selector, optional mode=file|dependencies|dependents, depth, limit, relation, relations, priority_threshold, and traversal budgets".to_owned(),
            output_description: "Updated session export after bounded graph expansion".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let mode_label = string_field(payload, "mode").unwrap_or_else(|| "dependencies".to_owned());
            let mode = parse_expand_mode(&mode_label)?;
            let before = inner.session.session_json()?;
            inner.session.expand(&target, mode, traversal_from_payload(payload))?;
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.walk",
                    &target,
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after).then(|| {
                    format!(
                        "session.walk(target={target},mode={mode_label},depth={})",
                        usize_field(payload, "depth").unwrap_or(1)
                    )
                }),
            })
        },
    );
}

fn register_session_walk_alias_tool(
    registry: &mut ToolRegistry,
    state: LiveGraphState,
    tool_name: &'static str,
    fixed_mode: &'static str,
    input_description: &'static str,
) {
    registry.register_with_result(
        ToolContract {
            name: tool_name.to_owned(),
            version: "v1".to_owned(),
            input_description: input_description.to_owned(),
            output_description: "Updated session export after bounded graph expansion".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut patched = payload.clone();
            if patched.get("mode").is_none() {
                patched["mode"] = Value::String(fixed_mode.to_owned());
            }
            let mut inner = state.lock()?;
            let target = resolved_target_selector(&patched, &inner.session)?;
            let before = inner.session.session_json()?;
            inner.session.expand(
                &target,
                parse_expand_mode(fixed_mode)?,
                traversal_from_payload(&patched),
            )?;
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    tool_name,
                    &target,
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after).then(|| {
                    format!(
                        "{tool_name}(target={target},depth={})",
                        usize_field(&patched, "depth").unwrap_or(1)
                    )
                }),
            })
        },
    );
}

fn register_session_focus_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.focus".to_owned(),
            version: "v1".to_owned(),
            input_description:
                "Payload with optional target selector; null or omission clears focus".to_owned(),
            output_description: "Updated session export after focus change".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let before = inner.session.session_json()?;
            match optional_target_selector(payload) {
                Some(target) => inner.session.focus(&normalize_selector(&target))?,
                None => inner.session.clear_focus()?,
            }
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.focus",
                    string_field(payload, "target").as_deref().unwrap_or("none"),
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after).then(|| {
                    format!(
                        "session.focus(target={})",
                        optional_target_selector(payload).unwrap_or_else(|| "none".to_owned())
                    )
                }),
            })
        },
    );
}

fn register_session_collapse_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.collapse".to_owned(),
            version: "v1".to_owned(),
            input_description:
                "Payload with optional target selector and include_descendants boolean.".to_owned(),
            output_description: "Updated session export after collapsing part of the working set"
                .to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let before = inner.session.session_json()?;
            inner.session.collapse(
                &target,
                bool_field(payload, "include_descendants").unwrap_or(false),
            )?;
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.collapse",
                    &target,
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after).then(|| {
                    format!(
                        "session.collapse(target={target},include_descendants={})",
                        bool_field(payload, "include_descendants").unwrap_or(false)
                    )
                }),
            })
        },
    );
}

fn register_session_pin_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.pin".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with optional target selector and pinned boolean."
                .to_owned(),
            output_description: "Updated session export after pinning or unpinning a node"
                .to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let pinned = bool_field(payload, "pinned").unwrap_or(true);
            let before = inner.session.session_json()?;
            inner.session.pin(&target, pinned)?;
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.pin",
                    &target,
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after)
                    .then(|| format!("session.pin(target={target},pinned={pinned})")),
            })
        },
    );
}

fn register_session_prune_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.prune".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with optional max_selected integer.".to_owned(),
            output_description: "Updated session export after pruning the working set".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let before = inner.session.session_json()?;
            inner.session.prune(usize_field(payload, "max_selected"));
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.prune",
                    "current_session",
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after).then(|| {
                    format!(
                        "session.prune(max_selected={:?})",
                        usize_field(payload, "max_selected")
                    )
                }),
            })
        },
    );
}

fn register_session_why_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.why".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with target selector".to_owned(),
            output_description: "Selection provenance and explanation for a node".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let explanation = inner.session.why_selected_json(&target)?;
            Ok(json!({
                "status": "ok",
                "target": target,
                "why": explanation,
            }))
        },
    );
}

fn register_session_explain_export_omission_tool(
    registry: &mut ToolRegistry,
    state: LiveGraphState,
) {
    registry.register(
        ToolContract {
            name: "session.explain_export_omission".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with optional target selector plus export tuning fields like max_tokens, compact, visible_levels, and class filters.".to_owned(),
            output_description: "Explanation for why a selector is omitted from the current export".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let explanation = inner.session.explain_export_omission_json(
                &target,
                usize_field(payload, "max_tokens"),
                bool_field(payload, "compact").unwrap_or(true),
                payload.get("include_rendered").and_then(|value| value.as_bool()),
                usize_field(payload, "visible_levels"),
                string_array(payload, "only_node_classes"),
                string_array(payload, "exclude_node_classes"),
                usize_field(payload, "max_frontier_actions"),
                usize_field(payload, "max_rendered_bytes"),
            )?;
            Ok(json!({
                "status": "ok",
                "target": target,
                "explanation": explanation,
            }))
        },
    );
}

fn register_session_why_pruned_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.why_pruned".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with optional target selector.".to_owned(),
            output_description: "Explanation for whether and why a selector was pruned".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            Ok(json!({
                "status": "ok",
                "target": target,
                "why_pruned": inner.session.why_pruned_json(&target)?,
            }))
        },
    );
}

fn register_session_export_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.export".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with optional max_tokens, compact, include_rendered, visible_levels, only_node_classes, exclude_node_classes, max_frontier_actions, and max_rendered_bytes".to_owned(),
            output_description: "Structured session export with nodes, edges, frontier, and heuristics".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let export = inner.session.export_json(
                usize_field(payload, "max_tokens"),
                bool_field(payload, "compact").unwrap_or(true),
                payload.get("include_rendered").and_then(|value| value.as_bool()),
                usize_field(payload, "visible_levels"),
                string_array(payload, "only_node_classes"),
                string_array(payload, "exclude_node_classes"),
                usize_field(payload, "max_frontier_actions"),
                usize_field(payload, "max_rendered_bytes"),
            )?;
            Ok(json!({
                "status": "ok",
                "export": export,
            }))
        },
    );
}

fn register_session_hydrate_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.hydrate".to_owned(),
            version: "v1".to_owned(),
            input_description:
                "Payload with target selector, optional padding, and hydration budgets".to_owned(),
            output_description: "Updated session export after source hydration".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let before = inner.session.session_json()?;
            inner.session.hydrate(
                &target,
                usize_field(payload, "padding").unwrap_or(2),
                budget_from_payload(payload),
            )?;
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.hydrate",
                    &target,
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after).then(|| {
                    format!(
                        "session.hydrate(target={target},padding={})",
                        usize_field(payload, "padding").unwrap_or(2)
                    )
                }),
            })
        },
    );
}

fn register_session_hydrate_alias_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "session.hydrate_source".to_owned(),
            version: "v1".to_owned(),
            input_description:
                "Payload with target selector, optional padding, and hydration budgets.".to_owned(),
            output_description: "Updated session export after source hydration".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let mut inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let before = inner.session.session_json()?;
            inner.session.hydrate(
                &target,
                usize_field(payload, "padding").unwrap_or(2),
                budget_from_payload(payload),
            )?;
            let after = inner.session.session_json()?;
            Ok(ToolExecutionResult {
                output: session_mutation_output(
                    "session.hydrate_source",
                    &target,
                    before != after,
                    &inner.session,
                    inner.render_max_tokens,
                )?,
                mutation_summary: (before != after).then(|| {
                    format!(
                        "session.hydrate_source(target={target},padding={})",
                        usize_field(payload, "padding").unwrap_or(2)
                    )
                }),
            })
        },
    );
}

fn register_session_recommendations_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.recommendations".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with optional top integer".to_owned(),
            output_description:
                "Structured frontier recommendations ranked by evidence gain and cost".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let recommendations = inner
                .session
                .recommendations_json(usize_field(payload, "top").unwrap_or(3))?;
            Ok(json!({
                "status": "ok",
                "recommendations": recommendations,
            }))
        },
    );
}

fn register_session_estimate_expand_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.estimate_expand".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with target selector, optional mode, depth, relation filters, priority threshold, and traversal budgets".to_owned(),
            output_description: "Estimated expansion cost and frontier growth without mutating the live session".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let mode_label = string_field(payload, "mode").unwrap_or_else(|| "dependencies".to_owned());
            let estimate = inner.session.estimate_expand_json(
                &target,
                parse_expand_mode(&mode_label)?,
                traversal_from_payload(payload),
            )?;
            Ok(json!({
                "status": "ok",
                "estimate": estimate,
            }))
        },
    );
}

fn register_session_estimate_hydrate_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.estimate_hydrate".to_owned(),
            version: "v1".to_owned(),
            input_description:
                "Payload with target selector, optional padding, and hydration budgets".to_owned(),
            output_description: "Estimated hydration cost without mutating the live session"
                .to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |payload| {
            let inner = state.lock()?;
            let target = resolved_target_selector(payload, &inner.session)?;
            let estimate = inner.session.estimate_hydrate_json(
                &target,
                usize_field(payload, "padding").unwrap_or(2),
                budget_from_payload(payload),
            )?;
            Ok(json!({
                "status": "ok",
                "estimate": estimate,
            }))
        },
    );
}

fn register_session_mutation_log_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.mutation_log".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload object with no required fields".to_owned(),
            output_description: "Chronological mutation telemetry for the current graph session"
                .to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |_| {
            let inner = state.lock()?;
            Ok(json!({
                "status": "ok",
                "mutation_log": inner.session.mutation_log_json()?,
            }))
        },
    );
}

fn register_session_event_log_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register(
        ToolContract {
            name: "session.event_log".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload object with no required fields".to_owned(),
            output_description: "Chronological event log for the current graph session".to_owned(),
        },
        |payload| payload.is_object(),
        is_object,
        move |_| {
            let inner = state.lock()?;
            Ok(json!({
                "status": "ok",
                "event_log": inner.session.event_log_json()?,
            }))
        },
    );
}

fn register_run_python_query_tool(registry: &mut ToolRegistry, state: LiveGraphState) {
    registry.register_with_result(
        ToolContract {
            name: "run_python_query".to_owned(),
            version: "v1".to_owned(),
            input_description: "Payload with code string plus optional bindings, include_export, export_kwargs, and limits".to_owned(),
            output_description: "PythonQueryTool-compatible result payload and any updated session state".to_owned(),
        },
        has_string("code"),
        is_object,
        move |payload| {
            ensure_ucp_python_runtime()?;
            let mut inner = state.lock()?;
            let before = inner.session.session_json()?;
            let output = execute_python_query(
                &inner.graph_snapshot_path,
                &before,
                payload.clone(),
            )?;
            let next_session_json = output
                .get("session_json")
                .and_then(|value| value.as_str())
                .ok_or_else(|| {
                    AppError::new(
                        ErrorCode::ProviderResponseInvalid,
                        "python query tool did not return session_json",
                        ErrorContext {
                            component: "graph_tools",
                            operation: "run_python_query",
                        },
                    )
                })?
                .to_owned();
            let result_payload = output
                .get("payload")
                .cloned()
                .ok_or_else(|| {
                    AppError::new(
                        ErrorCode::ProviderResponseInvalid,
                        "python query tool did not return payload",
                        ErrorContext {
                            component: "graph_tools",
                            operation: "run_python_query",
                        },
                    )
                })?;
            let changed = before != next_session_json;
            if changed {
                inner.session = inner.workspace.load_session_json(&next_session_json)?;
            }
            Ok(ToolExecutionResult {
                output: result_payload,
                mutation_summary: changed.then(|| "run_python_query".to_owned()),
            })
        },
    );
}

fn session_mutation_output(
    operation: &str,
    target: &str,
    changed: bool,
    session: &GraphSession,
    render_max_tokens: usize,
) -> Result<Value, AppError> {
    Ok(json!({
        "status": "ok",
        "operation": operation,
        "target": target,
        "changed": changed,
        "session_export": session.export_json(
            Some(render_max_tokens),
            true,
            Some(false),
            Some(3),
            Vec::new(),
            Vec::new(),
            Some(6),
            None,
        )?,
    }))
}

fn parse_detail_level(detail: Option<&str>) -> Result<RepresentationLevel, AppError> {
    match detail
        .unwrap_or("summary")
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "stub" | "skeleton" => Ok(RepresentationLevel::L0),
        "summary" | "card" | "symbol_card" => Ok(RepresentationLevel::L1),
        "neighborhood" => Ok(RepresentationLevel::L3),
        "full" | "source" => Ok(RepresentationLevel::L4),
        other => Err(AppError::new(
            ErrorCode::ConfigurationInvalid,
            format!("unsupported detail level '{other}'"),
            ErrorContext {
                component: "graph_tools",
                operation: "parse_detail_level",
            },
        )),
    }
}

fn detail_label(level: &RepresentationLevel) -> &'static str {
    match level {
        RepresentationLevel::L0 => "skeleton",
        RepresentationLevel::L1 => "summary",
        RepresentationLevel::L2 => "source",
        RepresentationLevel::L3 => "neighborhood",
        RepresentationLevel::L4 | RepresentationLevel::L5 => "source",
    }
}

fn parse_expand_mode(mode: &str) -> Result<CodeGraphExpandMode, AppError> {
    match mode {
        "file" => Ok(CodeGraphExpandMode::File),
        "dependencies" => Ok(CodeGraphExpandMode::Dependencies),
        "dependents" => Ok(CodeGraphExpandMode::Dependents),
        other => Err(AppError::new(
            ErrorCode::ConfigurationInvalid,
            format!("unsupported expand mode '{other}'"),
            ErrorContext {
                component: "graph_tools",
                operation: "parse_expand_mode",
            },
        )),
    }
}

fn traversal_from_payload(payload: &Value) -> CodeGraphTraversalConfig {
    let mut relation_filters = string_array(payload, "relations");
    if let Some(value) = string_field(payload, "relation") {
        if value != "*" {
            relation_filters.push(value);
        }
    }
    CodeGraphTraversalConfig {
        depth: usize_field(payload, "depth").unwrap_or(1).max(1),
        relation_filters,
        max_add: usize_field(payload, "limit"),
        priority_threshold: payload
            .get("priority_threshold")
            .and_then(|value| value.as_u64())
            .map(|value| value as u16),
        budget: budget_from_payload(payload),
    }
}

fn budget_from_payload(payload: &Value) -> Option<CodeGraphOperationBudget> {
    let nested = payload
        .get("traversal_budgets")
        .or_else(|| payload.get("limits"))
        .filter(|value| value.is_object());
    let budget = CodeGraphOperationBudget {
        max_depth: None,
        max_nodes_visited: usize_field(payload, "max_nodes_visited")
            .or_else(|| nested.and_then(|value| usize_field(value, "max_nodes_visited"))),
        max_nodes_added: usize_field(payload, "max_nodes_added")
            .or_else(|| nested.and_then(|value| usize_field(value, "max_nodes_added"))),
        max_hydrated_bytes: usize_field(payload, "max_hydrated_bytes")
            .or_else(|| nested.and_then(|value| usize_field(value, "max_hydrated_bytes"))),
        max_elapsed_ms: payload
            .get("max_elapsed_ms")
            .and_then(|value| value.as_u64())
            .or_else(|| {
                nested
                    .and_then(|value| value.get("max_elapsed_ms"))
                    .and_then(|value| value.as_u64())
            }),
        max_emitted_telemetry_events: usize_field(payload, "max_emitted_telemetry_events").or_else(
            || nested.and_then(|value| usize_field(value, "max_emitted_telemetry_events")),
        ),
    };
    if budget.max_nodes_visited.is_none()
        && budget.max_nodes_added.is_none()
        && budget.max_hydrated_bytes.is_none()
        && budget.max_elapsed_ms.is_none()
        && budget.max_emitted_telemetry_events.is_none()
    {
        None
    } else {
        Some(budget)
    }
}

fn execute_python_query(
    graph_snapshot_path: &Path,
    session_json: &str,
    arguments: Value,
) -> Result<Value, AppError> {
    let python = ensure_ucp_python_runtime()?;
    let input_path = unique_temp_path("graphbench-python-query-input.json");
    let payload = json!({
        "graph_snapshot_path": graph_snapshot_path.display().to_string(),
        "session_json": session_json,
        "arguments": arguments,
    });
    fs::write(
        &input_path,
        serde_json::to_vec(&payload).map_err(|source| {
            AppError::with_source(
                ErrorCode::PersistenceWriteFailed,
                "failed to serialize python query payload",
                ErrorContext {
                    component: "graph_tools",
                    operation: "write_python_query_payload",
                },
                source,
            )
        })?,
    )
    .map_err(|source| {
        AppError::with_source(
            ErrorCode::PersistenceWriteFailed,
            "failed to write python query payload",
            ErrorContext {
                component: "graph_tools",
                operation: "write_python_query_payload",
            },
            source,
        )
    })?;

    let output = Command::new(&python)
        .arg("-c")
        .arg(UCP_PYTHON_SCRIPT)
        .arg(&input_path)
        .output()
        .map_err(|source| {
            AppError::with_source(
                ErrorCode::ConfigurationInvalid,
                "failed to launch python query runtime",
                ErrorContext {
                    component: "graph_tools",
                    operation: "execute_python_query",
                },
                source,
            )
        })?;
    let _ = fs::remove_file(&input_path);
    if !output.status.success() {
        return Err(AppError::new(
            ErrorCode::ProviderResponseInvalid,
            format!(
                "python query runtime failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            ErrorContext {
                component: "graph_tools",
                operation: "execute_python_query",
            },
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|source| {
        AppError::with_source(
            ErrorCode::ProviderResponseInvalid,
            "python query runtime returned invalid JSON",
            ErrorContext {
                component: "graph_tools",
                operation: "parse_python_query_output",
            },
            source,
        )
    })
}

fn ensure_ucp_python_runtime() -> Result<PathBuf, AppError> {
    static READY: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    READY
        .get_or_init(|| {
            let python = PathBuf::from(UCP_PYTHON_VENV).join("bin/python");
            if !python.exists() {
                let status = Command::new("python3")
                    .arg("-m")
                    .arg("venv")
                    .arg(UCP_PYTHON_VENV)
                    .status()
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err("failed to create graphbench ucp-python virtualenv".to_owned());
                }
            }
            if python_imports_ucp(&python) {
                return Ok(python);
            }
            let status = Command::new(&python)
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg("-q")
                .arg("-U")
                .arg("pip")
                .arg("maturin")
                .status()
                .map_err(|error| error.to_string())?;
            if !status.success() {
                return Err(
                    "failed to install pip/maturin into graphbench ucp-python virtualenv"
                        .to_owned(),
                );
            }
            let status = Command::new(&python)
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg("-q")
                .arg("-e")
                .arg(UCP_PYTHON_ROOT)
                .status()
                .map_err(|error| error.to_string())?;
            if !status.success() {
                return Err("failed to install ucp-python into graphbench virtualenv".to_owned());
            }
            if python_imports_ucp(&python) {
                Ok(python)
            } else {
                Err("ucp-python still not importable after virtualenv install".to_owned())
            }
        })
        .clone()
        .map_err(|message| {
            AppError::new(
                ErrorCode::ConfigurationInvalid,
                message,
                ErrorContext {
                    component: "graph_tools",
                    operation: "ensure_ucp_python_runtime",
                },
            )
        })
}

fn python_imports_ucp(python: &Path) -> bool {
    Command::new(python)
        .arg("-c")
        .arg("import ucp")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn unique_temp_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{stamp}-{}.json", std::process::id()))
}

fn string_field(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn usize_field(payload: &Value, key: &str) -> Option<usize> {
    payload
        .get(key)
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
}

fn bool_field(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(|value| value.as_bool())
}

fn string_array(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect()
}

fn has_string(key: &'static str) -> impl Fn(&Value) -> bool {
    move |payload| payload.get(key).and_then(|value| value.as_str()).is_some()
}

fn optional_target_selector(payload: &Value) -> Option<String> {
    string_field(payload, "target")
        .or_else(|| string_field(payload, "selector"))
        .or_else(|| string_field(payload, "target_selector"))
}

fn normalize_selector(raw: &str) -> String {
    let raw = raw.trim().trim_matches(['[', ']']);
    if let Some(path) = raw.strip_prefix("path:") {
        return path.trim_start_matches('/').to_owned();
    }
    if let Some(path_like) = raw.strip_prefix("symbol:") {
        if !path_like.contains("::") {
            return path_like.trim_start_matches('/').to_owned();
        }
    }
    raw.trim_start_matches('/').to_owned()
}

fn resolved_target_selector(payload: &Value, session: &GraphSession) -> Result<String, AppError> {
    optional_target_selector(payload)
        .map(|value| normalize_selector(&value))
        .and_then(|value| {
            session
                .selector_for_short_id(value.trim_matches(['[', ']']))
                .or(Some(value))
        })
        .or_else(|| session.focus_selector())
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::ProviderResponseInvalid,
                "graph tool call requires a target selector or an existing focused node",
                ErrorContext {
                    component: "graph_tools",
                    operation: "resolved_target_selector",
                },
            )
        })
}

fn is_object(output: &Value) -> bool {
    output.is_object()
}
