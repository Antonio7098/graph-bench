use graphbench_core::{
    ContextWindowCompactionPolicy, ContextWindowStrategyPolicy, GraphDiscoveryMode, ProjectionMode,
    RereadMode, SectionTrimDirection, StrategyConfig, StrategySectionBudget,
    STRATEGY_CONFIG_SCHEMA_VERSION,
};

pub fn broad_graph_discovery() -> StrategyConfig {
    config(
        "graph.broad-discovery",
        GraphDiscoveryMode::BroadGraphDiscovery,
        ProjectionMode::Balanced,
        RereadMode::Allow,
        ContextWindowCompactionPolicy {
            history_recent_items: 5,
            summary_max_chars: 2_600,
            emergency_summary_max_chars: 1_000,
            deduplicate_tool_results: false,
        },
        vec![
            section_budget("base_runtime_instructions", 160, SectionTrimDirection::Tail),
            section_budget("response_contract", 140, SectionTrimDirection::Tail),
            section_budget("objective_state", 180, SectionTrimDirection::Tail),
            section_budget("selected_history", 220, SectionTrimDirection::Head),
            section_budget("active_code_windows", 320, SectionTrimDirection::Tail),
            section_budget("code_navigation_items", 420, SectionTrimDirection::Tail),
            section_budget("graph_relations", 260, SectionTrimDirection::Tail),
            section_budget("graph_frontier", 300, SectionTrimDirection::Tail),
            section_budget("tool_contracts", 2000, SectionTrimDirection::Tail),
        ],
    )
}

pub fn graph_then_targeted_lexical_read() -> StrategyConfig {
    config(
        "graph.targeted-lexical-read",
        GraphDiscoveryMode::GraphThenTargetedLexicalRead,
        ProjectionMode::Balanced,
        RereadMode::Allow,
        ContextWindowCompactionPolicy {
            history_recent_items: 4,
            summary_max_chars: 2_200,
            emergency_summary_max_chars: 900,
            deduplicate_tool_results: false,
        },
        vec![
            section_budget("base_runtime_instructions", 160, SectionTrimDirection::Tail),
            section_budget("response_contract", 140, SectionTrimDirection::Tail),
            section_budget("objective_state", 180, SectionTrimDirection::Tail),
            section_budget("selected_history", 220, SectionTrimDirection::Head),
            section_budget("active_code_windows", 440, SectionTrimDirection::Tail),
            section_budget("code_navigation_items", 260, SectionTrimDirection::Tail),
            section_budget("graph_relations", 180, SectionTrimDirection::Tail),
            section_budget("graph_frontier", 200, SectionTrimDirection::Tail),
            section_budget("tool_contracts", 2000, SectionTrimDirection::Tail),
        ],
    )
}

pub fn high_recall_projection() -> StrategyConfig {
    config(
        "projection.high-recall",
        GraphDiscoveryMode::GraphThenTargetedLexicalRead,
        ProjectionMode::HighRecall,
        RereadMode::Allow,
        ContextWindowCompactionPolicy {
            history_recent_items: 6,
            summary_max_chars: 3_200,
            emergency_summary_max_chars: 1_200,
            deduplicate_tool_results: false,
        },
        vec![
            section_budget("base_runtime_instructions", 180, SectionTrimDirection::Tail),
            section_budget("response_contract", 160, SectionTrimDirection::Tail),
            section_budget("objective_state", 200, SectionTrimDirection::Tail),
            section_budget("selected_history", 320, SectionTrimDirection::Head),
            section_budget("active_code_windows", 600, SectionTrimDirection::Tail),
            section_budget("code_navigation_items", 640, SectionTrimDirection::Tail),
            section_budget("graph_relations", 320, SectionTrimDirection::Tail),
            section_budget("graph_frontier", 360, SectionTrimDirection::Tail),
            section_budget("tool_contracts", 2000, SectionTrimDirection::Tail),
        ],
    )
}

pub fn minimal_projection() -> StrategyConfig {
    config(
        "projection.minimal",
        GraphDiscoveryMode::GraphThenTargetedLexicalRead,
        ProjectionMode::Minimal,
        RereadMode::Allow,
        ContextWindowCompactionPolicy {
            history_recent_items: 3,
            summary_max_chars: 1_400,
            emergency_summary_max_chars: 700,
            deduplicate_tool_results: true,
        },
        vec![
            section_budget("base_runtime_instructions", 120, SectionTrimDirection::Tail),
            section_budget("response_contract", 120, SectionTrimDirection::Tail),
            section_budget("objective_state", 140, SectionTrimDirection::Tail),
            section_budget("selected_history", 160, SectionTrimDirection::Head),
            section_budget("active_code_windows", 220, SectionTrimDirection::Tail),
            section_budget("code_navigation_items", 180, SectionTrimDirection::Tail),
            section_budget("graph_relations", 120, SectionTrimDirection::Tail),
            section_budget("graph_frontier", 140, SectionTrimDirection::Tail),
            section_budget("tool_contracts", 2000, SectionTrimDirection::Tail),
        ],
    )
}

pub fn strict_no_reread() -> StrategyConfig {
    config(
        "history.strict-no-reread",
        GraphDiscoveryMode::GraphThenTargetedLexicalRead,
        ProjectionMode::Balanced,
        RereadMode::StrictNoReread,
        ContextWindowCompactionPolicy {
            history_recent_items: 6,
            summary_max_chars: 2_600,
            emergency_summary_max_chars: 900,
            deduplicate_tool_results: true,
        },
        vec![
            section_budget("base_runtime_instructions", 180, SectionTrimDirection::Tail),
            section_budget("response_contract", 140, SectionTrimDirection::Tail),
            section_budget("objective_state", 180, SectionTrimDirection::Tail),
            section_budget("selected_history", 320, SectionTrimDirection::Head),
            section_budget("active_code_windows", 260, SectionTrimDirection::Tail),
            section_budget("code_navigation_items", 240, SectionTrimDirection::Tail),
            section_budget("graph_relations", 160, SectionTrimDirection::Tail),
            section_budget("graph_frontier", 180, SectionTrimDirection::Tail),
            section_budget("tool_contracts", 2000, SectionTrimDirection::Tail),
        ],
    )
}

pub fn preset_by_id(strategy_id: &str) -> Option<StrategyConfig> {
    match strategy_id {
        "graph.broad-discovery" => Some(broad_graph_discovery()),
        "graph.targeted-lexical-read" => Some(graph_then_targeted_lexical_read()),
        "projection.high-recall" => Some(high_recall_projection()),
        "projection.minimal" => Some(minimal_projection()),
        "history.strict-no-reread" => Some(strict_no_reread()),
        _ => None,
    }
}

fn config(
    strategy_id: &str,
    graph_discovery: GraphDiscoveryMode,
    projection: ProjectionMode,
    reread_policy: RereadMode,
    compaction: ContextWindowCompactionPolicy,
    section_budgets: Vec<StrategySectionBudget>,
) -> StrategyConfig {
    StrategyConfig {
        schema_version: STRATEGY_CONFIG_SCHEMA_VERSION,
        strategy_id: strategy_id.to_owned(),
        strategy_version: "v1".to_owned(),
        graph_discovery,
        projection,
        reread_policy,
        context_window: ContextWindowStrategyPolicy {
            compaction,
            section_budgets,
        },
    }
}

fn section_budget(
    section_id: &str,
    max_tokens: u32,
    trim_direction: SectionTrimDirection,
) -> StrategySectionBudget {
    StrategySectionBudget {
        section_id: section_id.to_owned(),
        max_tokens,
        trim_direction,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        broad_graph_discovery, graph_then_targeted_lexical_read, high_recall_projection,
        minimal_projection, strict_no_reread,
    };

    #[test]
    fn preset_strategies_validate() {
        for config in [
            broad_graph_discovery(),
            graph_then_targeted_lexical_read(),
            high_recall_projection(),
            minimal_projection(),
            strict_no_reread(),
        ] {
            assert!(config.validate().is_ok(), "{}", config.strategy_id);
        }
    }
}
