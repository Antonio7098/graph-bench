use crate::artifacts::{
    ContextIdentity, ContextObject, ContextObjectHashSet, ContextObjectKind, ContextProvenance,
    EvidenceMatch, FixtureManifest, LeaseState, OmittedCandidate, RenderedContextSection,
    RepresentationLevel,
};
use crate::error::{AppError, ErrorCode, ErrorContext};
use crate::fixtures::{FixtureResolution, sha256_of};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use ucm_core::PortableDocument;
use ucp_codegraph::{
    CodeGraphContextExport, CodeGraphDetailLevel, CodeGraphExpandMode, CodeGraphExportConfig,
    CodeGraphFindQuery, CodeGraphNavigator, CodeGraphNavigatorSession, CodeGraphOperationBudget,
    CodeGraphRenderConfig, CodeGraphTraversalConfig, approximate_prompt_tokens,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphPromptHooks {
    pub active_code_windows: RenderedContextSection,
    pub code_navigation_items: RenderedContextSection,
    pub graph_relations: RenderedContextSection,
    pub graph_frontier: RenderedContextSection,
    pub omitted_candidates: Vec<OmittedCandidate>,
    pub context_objects: Vec<ContextObject>,
    pub rendered_context: String,
    pub context_hash: String,
}

#[derive(Debug)]
pub struct GraphWorkspace {
    navigator: CodeGraphNavigator,
    fixture: FixtureManifest,
    graph_snapshot_hash: String,
}

impl GraphWorkspace {
    pub fn load(
        fixture: FixtureManifest,
        resolution: &FixtureResolution,
    ) -> Result<Self, AppError> {
        if fixture.graph.snapshot_format != "codegraph_portable_document" {
            return Err(AppError::new(
                ErrorCode::GraphSnapshotMissing,
                format!(
                    "unsupported graph snapshot format '{}'",
                    fixture.graph.snapshot_format
                ),
                ErrorContext {
                    component: "graph",
                    operation: "load_graph_snapshot",
                },
            ));
        }

        let raw = fs::read_to_string(&resolution.snapshot_path).map_err(|source| {
            AppError::with_source(
                ErrorCode::GraphSnapshotMissing,
                format!(
                    "failed to read graph snapshot at {}",
                    resolution.snapshot_path.display()
                ),
                ErrorContext {
                    component: "graph",
                    operation: "read_graph_snapshot",
                },
                source,
            )
        })?;
        let actual_hash = sha256_of(raw.as_bytes());
        if actual_hash != fixture.graph.snapshot_id {
            return Err(AppError::new(
                ErrorCode::GraphSnapshotMissing,
                format!(
                    "graph snapshot hash mismatch: expected {}, got {}",
                    fixture.graph.snapshot_id, actual_hash
                ),
                ErrorContext {
                    component: "graph",
                    operation: "validate_graph_snapshot",
                },
            ));
        }

        let portable: PortableDocument = serde_json::from_str(&raw).map_err(|source| {
            AppError::with_source(
                ErrorCode::GraphSnapshotMissing,
                "failed to parse portable graph snapshot",
                ErrorContext {
                    component: "graph",
                    operation: "parse_graph_snapshot",
                },
                source,
            )
        })?;
        let document = portable.to_document().map_err(|source| {
            AppError::with_source(
                ErrorCode::GraphSnapshotMissing,
                "failed to reconstruct codegraph document from portable snapshot",
                ErrorContext {
                    component: "graph",
                    operation: "rebuild_graph_snapshot",
                },
                source,
            )
        })?;

        Ok(Self {
            navigator: CodeGraphNavigator::new(document),
            fixture,
            graph_snapshot_hash: actual_hash,
        })
    }

    pub fn session(&self) -> GraphSession {
        GraphSession {
            fixture_id: self.fixture.fixture_id.clone(),
            graph_snapshot_id: self.graph_snapshot_hash.clone(),
            session: self.navigator.session(),
            exact_proof_hydrations: BTreeSet::new(),
        }
    }

    pub fn load_session_json(&self, payload: &str) -> Result<GraphSession, AppError> {
        let session = self
            .navigator
            .load_session_json(payload)
            .map_err(graph_error("load_session_json"))?;
        Ok(GraphSession {
            fixture_id: self.fixture.fixture_id.clone(),
            graph_snapshot_id: self.graph_snapshot_hash.clone(),
            session,
            exact_proof_hydrations: BTreeSet::new(),
        })
    }

    pub fn snapshot_id(&self) -> &str {
        &self.graph_snapshot_hash
    }
}

#[derive(Debug)]
pub struct GraphSession {
    fixture_id: String,
    graph_snapshot_id: String,
    session: CodeGraphNavigatorSession,
    exact_proof_hydrations: BTreeSet<String>,
}

impl GraphSession {
    pub fn seed_overview(&mut self, max_depth: Option<usize>) {
        self.session.seed_overview(max_depth);
    }

    pub fn focus(&mut self, selector: &str) -> Result<(), AppError> {
        self.session
            .focus(Some(selector))
            .map_err(graph_error("focus_session"))?;
        Ok(())
    }

    pub fn select(
        &mut self,
        selector: &str,
        representation_level: RepresentationLevel,
    ) -> Result<(), AppError> {
        self.session
            .select(selector, detail_for_representation(representation_level))
            .map_err(graph_error("select_session"))?;
        Ok(())
    }

    pub fn expand(
        &mut self,
        selector: &str,
        mode: CodeGraphExpandMode,
        traversal: CodeGraphTraversalConfig,
    ) -> Result<(), AppError> {
        self.session
            .expand(selector, mode, &traversal)
            .map_err(graph_error("expand"))?;
        Ok(())
    }

    pub fn expand_file(&mut self, selector: &str, depth: usize) -> Result<(), AppError> {
        self.expand(
            selector,
            CodeGraphExpandMode::File,
            CodeGraphTraversalConfig {
                depth,
                ..CodeGraphTraversalConfig::default()
            },
        )
    }

    pub fn expand_dependencies(&mut self, selector: &str, depth: usize) -> Result<(), AppError> {
        self.expand(
            selector,
            CodeGraphExpandMode::Dependencies,
            CodeGraphTraversalConfig {
                depth,
                ..CodeGraphTraversalConfig::default()
            },
        )
    }

    pub fn expand_dependents(&mut self, selector: &str, depth: usize) -> Result<(), AppError> {
        self.expand(
            selector,
            CodeGraphExpandMode::Dependents,
            CodeGraphTraversalConfig {
                depth,
                ..CodeGraphTraversalConfig::default()
            },
        )
    }

    pub fn hydrate_exact_proof(&mut self, selector: &str, padding: usize) -> Result<(), AppError> {
        self.hydrate(
            selector,
            padding,
            Some(CodeGraphOperationBudget {
                max_nodes_visited: Some(1),
                max_emitted_telemetry_events: Some(4),
                ..CodeGraphOperationBudget::default()
            }),
        )?;
        self.exact_proof_hydrations.insert(selector.to_owned());
        Ok(())
    }

    pub fn hydrate(
        &mut self,
        selector: &str,
        padding: usize,
        budget: Option<CodeGraphOperationBudget>,
    ) -> Result<(), AppError> {
        self.session
            .hydrate_source_with_budget(selector, padding, budget)
            .map_err(graph_error("hydrate"))?;
        Ok(())
    }

    pub fn clear_focus(&mut self) -> Result<(), AppError> {
        self.session
            .focus(None)
            .map_err(graph_error("clear_focus_session"))?;
        Ok(())
    }

    pub fn collapse(&mut self, selector: &str, include_descendants: bool) -> Result<(), AppError> {
        self.session
            .collapse(selector, include_descendants)
            .map_err(graph_error("collapse"))?;
        Ok(())
    }

    pub fn pin(&mut self, selector: &str, pinned: bool) -> Result<(), AppError> {
        self.session
            .pin(selector, pinned)
            .map_err(graph_error("pin"))?;
        Ok(())
    }

    pub fn prune(&mut self, max_selected: Option<usize>) {
        self.session.prune(max_selected);
    }

    pub fn session_json(&self) -> Result<String, AppError> {
        self.session
            .to_json()
            .map_err(graph_error("session_to_json"))
    }

    pub fn render_for_harness(&self, max_tokens: usize) -> GraphPromptHooks {
        let render = CodeGraphRenderConfig::for_max_tokens(max_tokens);
        let export = self
            .session
            .export(&render, &CodeGraphExportConfig::compact());

        let context_objects = export
            .nodes
            .iter()
            .map(|node| context_object_from_export(&self.fixture_id, &self.graph_snapshot_id, node))
            .collect::<Vec<_>>();
        let omitted_candidates = export
            .omissions
            .details
            .iter()
            .map(|detail| OmittedCandidate {
                candidate_id: detail
                    .label
                    .clone()
                    .or_else(|| detail.short_id.clone())
                    .unwrap_or_else(|| "unknown".to_owned()),
                reason: detail.explanation.clone(),
            })
            .collect::<Vec<_>>();

        let active_code_windows_content = export
            .nodes
            .iter()
            .filter_map(|node| node.hydrated_source.as_ref())
            .map(|excerpt| {
                format!(
                    "{}\n{}\n",
                    excerpt.display,
                    excerpt.snippet.trim_end_matches('\n')
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let active_code_windows = rendered_section(
            "active_code_windows",
            "Active Code Windows",
            &active_code_windows_content,
        );

        let code_navigation_content = export
            .nodes
            .iter()
            .map(|node| {
                format!(
                    "- {} | kind={} | logical_key={} | path={} | detail={:?}",
                    node.label,
                    node.node_class,
                    node.logical_key.clone().unwrap_or_else(|| "-".to_owned()),
                    node.path.clone().unwrap_or_else(|| "-".to_owned()),
                    node.detail_level
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let code_navigation_items = rendered_section(
            "code_navigation_items",
            "Code-Navigation Items",
            &code_navigation_content,
        );

        let graph_relations_content = render_graph_relations(&export);
        let graph_relations = rendered_section(
            "graph_relations",
            "Graph Relations",
            &graph_relations_content,
        );

        let graph_frontier_content = render_graph_frontier(&export);
        let graph_frontier =
            rendered_section("graph_frontier", "Graph Frontier", &graph_frontier_content);

        let rendered_context = [
            section_to_prompt(&active_code_windows),
            section_to_prompt(&code_navigation_items),
            section_to_prompt(&graph_relations),
            section_to_prompt(&graph_frontier),
        ]
        .join("\n\n");

        GraphPromptHooks {
            active_code_windows,
            code_navigation_items,
            graph_relations,
            graph_frontier,
            omitted_candidates,
            context_objects,
            rendered_context: rendered_context.clone(),
            context_hash: sha256_of(rendered_context.as_bytes()),
        }
    }

    pub fn export_for_prompt(&self, max_tokens: usize) -> CodeGraphContextExport {
        self.session.export(
            &CodeGraphRenderConfig::for_max_tokens(max_tokens),
            &CodeGraphExportConfig::compact(),
        )
    }

    pub fn focus_selector(&self) -> Option<String> {
        let export = self.export_for_prompt(512);
        let focus = export.focus?;
        export
            .nodes
            .iter()
            .find(|node| node.block_id == focus)
            .and_then(|node| {
                node.logical_key
                    .clone()
                    .or_else(|| node.path.clone())
                    .or_else(|| Some(node.label.clone()))
            })
    }

    pub fn selector_for_short_id(&self, short_id: &str) -> Option<String> {
        let export = self.export_for_prompt(1024);
        export
            .nodes
            .iter()
            .find(|node| node.short_id == short_id)
            .and_then(|node| {
                node.logical_key
                    .clone()
                    .or_else(|| node.path.clone())
                    .or_else(|| Some(node.label.clone()))
            })
    }

    pub fn explain_selector_json(&self, selector: &str) -> Result<Value, AppError> {
        as_json_value(&self.session.explain_selector(selector), "explain_selector")
    }

    pub fn find_nodes_json(&self, query: CodeGraphFindQuery) -> Result<Value, AppError> {
        let matches = self
            .session
            .find_nodes(&query)
            .map_err(graph_error("find_nodes"))?;
        as_json_value(&matches, "find_nodes")
    }

    pub fn path_between_json(
        &self,
        start_selector: &str,
        end_selector: &str,
        max_hops: usize,
    ) -> Result<Value, AppError> {
        let path = self
            .session
            .path_between(start_selector, end_selector, max_hops)
            .map_err(graph_error("path_between"))?;
        as_json_value(&path, "path_between")
    }

    pub fn why_selected_json(&self, selector: &str) -> Result<Value, AppError> {
        let explanation = self
            .session
            .why_selected(selector)
            .map_err(graph_error("why_selected"))?;
        as_json_value(&explanation, "why_selected")
    }

    pub fn export_json(
        &self,
        max_tokens: Option<usize>,
        compact: bool,
        include_rendered: Option<bool>,
        visible_levels: Option<usize>,
        only_node_classes: Vec<String>,
        exclude_node_classes: Vec<String>,
        max_frontier_actions: Option<usize>,
        max_rendered_bytes: Option<usize>,
    ) -> Result<Value, AppError> {
        let mut render = max_tokens
            .map(CodeGraphRenderConfig::for_max_tokens)
            .unwrap_or_default();
        render.max_rendered_bytes = max_rendered_bytes;
        let mut export = if compact {
            CodeGraphExportConfig::compact()
        } else {
            CodeGraphExportConfig::default()
        };
        if let Some(value) = include_rendered {
            export.include_rendered = value;
        }
        export.visible_levels = visible_levels;
        export.only_node_classes = only_node_classes;
        export.exclude_node_classes = exclude_node_classes;
        if let Some(value) = max_frontier_actions {
            export.max_frontier_actions = value.max(1);
        }
        as_json_value(&self.session.export(&render, &export), "export")
    }

    pub fn mutation_log_json(&self) -> Result<Value, AppError> {
        as_json_value(self.session.mutation_log(), "mutation_log")
    }

    pub fn event_log_json(&self) -> Result<Value, AppError> {
        as_json_value(self.session.event_log(), "event_log")
    }

    pub fn recommendations_json(&self, top: usize) -> Result<Value, AppError> {
        as_json_value(&self.session.recommendations(top), "recommendations")
    }

    pub fn estimate_expand_json(
        &self,
        selector: &str,
        mode: CodeGraphExpandMode,
        traversal: CodeGraphTraversalConfig,
    ) -> Result<Value, AppError> {
        let estimate = self
            .session
            .estimate_expand(selector, mode, &traversal)
            .map_err(graph_error("estimate_expand"))?;
        as_json_value(&estimate, "estimate_expand")
    }

    pub fn estimate_hydrate_json(
        &self,
        selector: &str,
        padding: usize,
        budget: Option<CodeGraphOperationBudget>,
    ) -> Result<Value, AppError> {
        let estimate = self
            .session
            .estimate_hydrate(selector, padding, budget)
            .map_err(graph_error("estimate_hydrate"))?;
        as_json_value(&estimate, "estimate_hydrate")
    }

    pub fn explain_export_omission_json(
        &self,
        selector: &str,
        max_tokens: Option<usize>,
        compact: bool,
        include_rendered: Option<bool>,
        visible_levels: Option<usize>,
        only_node_classes: Vec<String>,
        exclude_node_classes: Vec<String>,
        max_frontier_actions: Option<usize>,
        max_rendered_bytes: Option<usize>,
    ) -> Result<Value, AppError> {
        let mut render = max_tokens
            .map(CodeGraphRenderConfig::for_max_tokens)
            .unwrap_or_default();
        render.max_rendered_bytes = max_rendered_bytes;
        let mut export = if compact {
            CodeGraphExportConfig::compact()
        } else {
            CodeGraphExportConfig::default()
        };
        if let Some(value) = include_rendered {
            export.include_rendered = value;
        }
        export.visible_levels = visible_levels;
        export.only_node_classes = only_node_classes;
        export.exclude_node_classes = exclude_node_classes;
        if let Some(value) = max_frontier_actions {
            export.max_frontier_actions = value.max(1);
        }
        let explanation = self
            .session
            .explain_export_omission(selector, &render, &export)
            .map_err(graph_error("explain_export_omission"))?;
        as_json_value(&explanation, "explain_export_omission")
    }

    pub fn why_pruned_json(&self, selector: &str) -> Result<Value, AppError> {
        let explanation = self
            .session
            .why_pruned(selector)
            .map_err(graph_error("why_pruned"))?;
        as_json_value(&explanation, "why_pruned")
    }

    pub fn apply_recommended_actions_json(
        &mut self,
        top: usize,
        padding: usize,
        depth: Option<usize>,
        max_add: Option<usize>,
        priority_threshold: Option<u16>,
    ) -> Result<Value, AppError> {
        let result = self
            .session
            .apply_recommended_actions(top, padding, depth, max_add, priority_threshold)
            .map_err(graph_error("apply_recommended_actions"))?;
        as_json_value(&result, "apply_recommended_actions")
    }
}

pub fn persist_codegraph_snapshot(
    repository_root: impl AsRef<Path>,
    commit_hash: &str,
    destination: impl AsRef<Path>,
) -> Result<String, AppError> {
    let build = ucp_codegraph::build_code_graph(&ucp_codegraph::CodeGraphBuildInput {
        repository_path: repository_root.as_ref().to_path_buf(),
        commit_hash: commit_hash.to_owned(),
        config: Default::default(),
    })
    .map_err(graph_error("build_codegraph"))?;
    let portable = build.document.to_portable();
    let serialized = serde_json::to_string_pretty(&portable).map_err(|source| {
        AppError::with_source(
            ErrorCode::GraphSnapshotMissing,
            "failed to serialize portable graph snapshot",
            ErrorContext {
                component: "graph",
                operation: "serialize_graph_snapshot",
            },
            source,
        )
    })?;
    fs::write(destination.as_ref(), &serialized).map_err(|source| {
        AppError::with_source(
            ErrorCode::PersistenceWriteFailed,
            format!(
                "failed to write graph snapshot to {}",
                destination.as_ref().display()
            ),
            ErrorContext {
                component: "graph",
                operation: "persist_graph_snapshot",
            },
            source,
        )
    })?;
    Ok(sha256_of(serialized.as_bytes()))
}

fn rendered_section(section_id: &str, title: &str, content: &str) -> RenderedContextSection {
    let content = if content.trim().is_empty() {
        "No visible items.".to_owned()
    } else {
        content.to_owned()
    };
    RenderedContextSection {
        section_id: section_id.to_owned(),
        schema_version: crate::artifacts::CONTEXT_WINDOW_SECTION_SCHEMA_VERSION,
        title: title.to_owned(),
        content: content.clone(),
        byte_count: content.len() as u32,
        token_count: approximate_prompt_tokens(&content),
    }
}

fn section_to_prompt(section: &RenderedContextSection) -> String {
    format!("## {}\n{}", section.title, section.content)
}

fn render_graph_relations(export: &CodeGraphContextExport) -> String {
    if export.edges.is_empty() {
        return "No explicit graph relations are currently visible.".to_owned();
    }

    let labels = export
        .nodes
        .iter()
        .map(|node| (node.block_id, (node.label.as_str(), node.short_id.as_str())))
        .collect::<HashMap<_, _>>();

    export
        .edges
        .iter()
        .map(|edge| {
            let (source_label, source_short_id) = labels
                .get(&edge.source)
                .copied()
                .unwrap_or(("-", edge.source_short_id.as_str()));
            let (target_label, target_short_id) = labels
                .get(&edge.target)
                .copied()
                .unwrap_or(("-", edge.target_short_id.as_str()));
            let multiplicity = if edge.multiplicity > 1 {
                format!(" x{}", edge.multiplicity)
            } else {
                String::new()
            };
            format!(
                "- [{source_short_id}] {source_label} --{}{}--> [{target_short_id}] {target_label}",
                edge.relation, multiplicity
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_graph_frontier(export: &CodeGraphContextExport) -> String {
    let mut lines = vec![format!(
        "Focus: {} | visible_nodes={} | hidden_unreachable={} | omitted_symbols={}",
        export
            .focus_label
            .clone()
            .unwrap_or_else(|| "none".to_owned()),
        export.visible_node_count,
        export.hidden_unreachable_count,
        export.omitted_symbol_count
    )];
    if !export.hidden_levels.is_empty() {
        lines.push(format!(
            "Hidden levels: {}",
            export
                .hidden_levels
                .iter()
                .map(|level| {
                    format!(
                        "level={} count={} relation={} direction={}",
                        level.level,
                        level.count,
                        level.relation.clone().unwrap_or_else(|| "-".to_owned()),
                        level.direction.clone().unwrap_or_else(|| "-".to_owned())
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }
    if !export.frontier.is_empty() {
        lines.push("Recommended frontier actions:".to_owned());
        lines.extend(export.frontier.iter().map(|action| {
            let tool_hint = match action.action.as_str() {
                "expand_file" => "session.walk(mode=file)",
                "expand_dependencies" => "session.walk(mode=dependencies)",
                "expand_dependents" => "session.walk(mode=dependents)",
                "hydrate_source" => "session.hydrate(...)",
                "collapse" => "session.export(...) or session.focus(...)",
                _ => "graph/session tool",
            };
            format!(
                "- {} [{}] relation={} direction={} candidates={} priority={} via {} :: {}",
                action.action,
                action.short_id,
                action.relation.clone().unwrap_or_else(|| "-".to_owned()),
                action.direction.clone().unwrap_or_else(|| "-".to_owned()),
                action.candidate_count,
                action.priority,
                tool_hint,
                action.description
            )
        }));
    } else {
        lines.push("Recommended frontier actions: none visible.".to_owned());
    }
    if let Some(recommendation) = &export.heuristics.recommended_next_action {
        lines.push(format!(
            "Heuristic next action: {} [{}] priority={} candidates={} :: {}",
            recommendation.action,
            recommendation.short_id,
            recommendation.priority,
            recommendation.candidate_count,
            recommendation.description
        ));
    }
    if !export.heuristics.reasons.is_empty() {
        lines.push(format!(
            "Heuristic notes: {}",
            export.heuristics.reasons.join(" | ")
        ));
    }
    lines.join("\n")
}

fn as_json_value(
    serializable: &(impl Serialize + ?Sized),
    operation: &'static str,
) -> Result<Value, AppError> {
    serde_json::to_value(serializable).map_err(|source| {
        AppError::with_source(
            ErrorCode::SchemaValidationFailed,
            "failed to serialize graph result",
            ErrorContext {
                component: "graph",
                operation,
            },
            source,
        )
    })
}

fn detail_for_representation(level: RepresentationLevel) -> CodeGraphDetailLevel {
    match level {
        RepresentationLevel::L0 => CodeGraphDetailLevel::Skeleton,
        RepresentationLevel::L1 => CodeGraphDetailLevel::SymbolCard,
        RepresentationLevel::L2 => CodeGraphDetailLevel::Source,
        RepresentationLevel::L3 => CodeGraphDetailLevel::Neighborhood,
        RepresentationLevel::L4 | RepresentationLevel::L5 => CodeGraphDetailLevel::Source,
    }
}

fn representation_for_node(
    node: &ucp_codegraph::CodeGraphContextNodeExport,
) -> RepresentationLevel {
    if let Some(path) = &node.path {
        if node.hydrated_source.is_some() && path.ends_with(".rs") {
            return RepresentationLevel::L4;
        }
        if node.hydrated_source.is_some() {
            return RepresentationLevel::L2;
        }
    }

    match node.detail_level {
        CodeGraphDetailLevel::Skeleton => RepresentationLevel::L0,
        CodeGraphDetailLevel::SymbolCard => RepresentationLevel::L1,
        CodeGraphDetailLevel::Neighborhood => RepresentationLevel::L3,
        CodeGraphDetailLevel::Source => RepresentationLevel::L2,
    }
}

fn context_object_from_export(
    fixture_id: &str,
    graph_snapshot_id: &str,
    node: &ucp_codegraph::CodeGraphContextNodeExport,
) -> ContextObject {
    let object_hash = sha256_of(
        format!(
            "{}:{}:{}",
            fixture_id,
            node.logical_key
                .clone()
                .unwrap_or_else(|| node.label.clone()),
            node.detail_level as u8
        )
        .as_bytes(),
    );

    ContextObject {
        context_object_id: format!("{}.{}", fixture_id, node.short_id),
        schema_version: crate::artifacts::CONTEXT_OBJECT_SCHEMA_VERSION,
        graph_snapshot_id: graph_snapshot_id.to_owned(),
        kind: match node.node_class.as_str() {
            "symbol" => ContextObjectKind::Symbol,
            "file" => ContextObjectKind::FileRegion,
            _ => ContextObjectKind::DependencyNeighborhood,
        },
        identity: ContextIdentity {
            logical_key: node.logical_key.clone(),
            path: node.path.clone(),
            symbol: node.symbol_name.clone(),
        },
        representation_level: representation_for_node(node),
        provenance: ContextProvenance {
            source_kind: node
                .origin
                .as_ref()
                .map(|origin| format!("{:?}", origin.kind).to_lowercase())
                .unwrap_or_else(|| "manual".to_owned()),
            anchor_id: node
                .origin
                .as_ref()
                .and_then(|origin| origin.anchor)
                .map(|value| value.to_string())
                .unwrap_or_else(|| node.block_id.to_string()),
        },
        relevance_score: i32::from(node.relevance_score),
        lease_state: if node.pinned {
            LeaseState::Granted
        } else {
            LeaseState::Expiring
        },
        evidence_matches: node
            .logical_key
            .as_ref()
            .map(|logical_key| {
                vec![EvidenceMatch {
                    fact_id: logical_key.clone(),
                }]
            })
            .unwrap_or_default(),
        hashes: ContextObjectHashSet { object_hash },
    }
}

fn graph_error(operation: &'static str) -> impl Fn(anyhow::Error) -> AppError {
    move |source| {
        AppError::new(
            ErrorCode::GraphSnapshotMissing,
            format!("graph operation failed: {source}"),
            ErrorContext {
                component: "graph",
                operation,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{GraphWorkspace, persist_codegraph_snapshot};
    use crate::{RepresentationLevel, fixtures::FixtureRepository};
    use std::path::Path;

    #[test]
    fn graph_workspace_loads_fixture_snapshot_and_exports_prompt_hooks() {
        let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/graphbench-internal/fixture.json");
        let fixture_repository = FixtureRepository;
        let (fixture, resolution) = fixture_repository
            .load(&manifest_path)
            .expect("fixture should load");
        let workspace =
            GraphWorkspace::load(fixture, &resolution).expect("graph workspace should load");
        let mut session = workspace.session();
        session.seed_overview(Some(2));
        session
            .select(
                "crates/graphbench-core/src/artifacts.rs",
                RepresentationLevel::L1,
            )
            .expect("select path");
        session
            .hydrate_exact_proof("crates/graphbench-core/src/artifacts.rs", 2)
            .expect("hydrate proof");
        let hooks = session.render_for_harness(1024);
        assert!(!hooks.context_objects.is_empty());
        assert!(!hooks.context_hash.is_empty());
        assert_eq!(
            hooks.code_navigation_items.section_id,
            "code_navigation_items"
        );
    }

    #[test]
    fn snapshot_persistence_generates_hash_addressed_artifact() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let destination = std::env::temp_dir().join("graphbench-generated.snapshot.json");
        let hash = persist_codegraph_snapshot(
            repository_root,
            "1111111111111111111111111111111111111111",
            &destination,
        )
        .expect("snapshot should be persisted");
        assert!(hash.starts_with("sha256:"));
        assert!(destination.exists());
        let _ = std::fs::remove_file(destination);
    }
}
