use crate::error::{AppError, ErrorCode, ErrorContext};
use serde::{Deserialize, Serialize};

pub const STRATEGY_CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphDiscoveryMode {
    BroadGraphDiscovery,
    GraphThenTargetedLexicalRead,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMode {
    Balanced,
    HighRecall,
    Minimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RereadMode {
    Allow,
    StrictNoReread,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SectionTrimDirection {
    Head,
    Tail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategySectionBudget {
    pub section_id: String,
    pub max_tokens: u32,
    pub trim_direction: SectionTrimDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextWindowCompactionPolicy {
    pub history_recent_items: u32,
    pub summary_max_chars: u32,
    pub emergency_summary_max_chars: u32,
    pub deduplicate_tool_results: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextWindowStrategyPolicy {
    pub compaction: ContextWindowCompactionPolicy,
    pub section_budgets: Vec<StrategySectionBudget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyConfig {
    pub schema_version: u32,
    pub strategy_id: String,
    pub strategy_version: String,
    pub graph_discovery: GraphDiscoveryMode,
    pub projection: ProjectionMode,
    pub reread_policy: RereadMode,
    pub context_window: ContextWindowStrategyPolicy,
}

impl StrategyConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != STRATEGY_CONFIG_SCHEMA_VERSION {
            return Err(validation_error(
                "unsupported strategy config schema version",
                "validate",
            ));
        }

        validate_strategy_id(&self.strategy_id)?;
        validate_strategy_version(&self.strategy_version)?;

        if self.context_window.compaction.history_recent_items == 0 {
            return Err(validation_error(
                "strategy compaction must retain at least one recent history item",
                "validate",
            ));
        }
        if self.context_window.compaction.summary_max_chars == 0
            || self.context_window.compaction.emergency_summary_max_chars == 0
        {
            return Err(validation_error(
                "strategy compaction summaries must allow at least one character",
                "validate",
            ));
        }
        if self.context_window.section_budgets.is_empty() {
            return Err(validation_error(
                "strategy must declare section budgets",
                "validate",
            ));
        }

        for budget in &self.context_window.section_budgets {
            validate_section_budget(budget)?;
        }

        Ok(())
    }
}

pub fn validate_strategy_id(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(validation_error(
            "strategy_id must not be empty",
            "validate_strategy_id",
        ));
    }

    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() < 2 {
        return Err(validation_error(
            "strategy_id must use <family>.<variant> format",
            "validate_strategy_id",
        ));
    }
    for segment in segments {
        if segment.is_empty()
            || !segment.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '-' | '_')
            })
        {
            return Err(validation_error(
                "strategy_id segments must be lowercase ascii identifiers",
                "validate_strategy_id",
            ));
        }
    }

    Ok(())
}

pub fn validate_strategy_version(value: &str) -> Result<(), AppError> {
    let Some(version) = value.strip_prefix('v') else {
        return Err(validation_error(
            "strategy_version must use v<major>[.<minor>[.<patch>]] format",
            "validate_strategy_version",
        ));
    };

    let parts = version.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 3 {
        return Err(validation_error(
            "strategy_version must have between one and three numeric parts",
            "validate_strategy_version",
        ));
    }
    if parts
        .iter()
        .any(|part| part.is_empty() || !part.chars().all(|character| character.is_ascii_digit()))
    {
        return Err(validation_error(
            "strategy_version parts must be numeric",
            "validate_strategy_version",
        ));
    }

    Ok(())
}

fn validate_section_budget(budget: &StrategySectionBudget) -> Result<(), AppError> {
    if budget.section_id.trim().is_empty() {
        return Err(validation_error(
            "strategy section budgets require a section_id",
            "validate_section_budget",
        ));
    }
    if !budget
        .section_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(validation_error(
            "strategy section budget ids must be ascii identifiers",
            "validate_section_budget",
        ));
    }
    if budget.max_tokens == 0 {
        return Err(validation_error(
            "strategy section budgets must allow at least one token",
            "validate_section_budget",
        ));
    }

    Ok(())
}

fn validation_error(message: &'static str, operation: &'static str) -> AppError {
    AppError::new(
        ErrorCode::SchemaValidationFailed,
        message,
        ErrorContext {
            component: "strategy_config",
            operation,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ContextWindowCompactionPolicy, ContextWindowStrategyPolicy, GraphDiscoveryMode,
        ProjectionMode, RereadMode, STRATEGY_CONFIG_SCHEMA_VERSION, SectionTrimDirection,
        StrategyConfig, StrategySectionBudget, validate_strategy_id, validate_strategy_version,
    };

    #[test]
    fn strategy_config_validates_with_versioned_id() {
        let config = StrategyConfig {
            schema_version: STRATEGY_CONFIG_SCHEMA_VERSION,
            strategy_id: "graph.broad-discovery".to_owned(),
            strategy_version: "v1.0".to_owned(),
            graph_discovery: GraphDiscoveryMode::BroadGraphDiscovery,
            projection: ProjectionMode::Balanced,
            reread_policy: RereadMode::Allow,
            context_window: ContextWindowStrategyPolicy {
                compaction: ContextWindowCompactionPolicy {
                    history_recent_items: 4,
                    summary_max_chars: 2_000,
                    emergency_summary_max_chars: 800,
                    deduplicate_tool_results: false,
                },
                section_budgets: vec![StrategySectionBudget {
                    section_id: "selected_history".to_owned(),
                    max_tokens: 256,
                    trim_direction: SectionTrimDirection::Head,
                }],
            },
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn strategy_id_requires_family_and_variant() {
        assert!(validate_strategy_id("baseline").is_err());
        assert!(validate_strategy_id("graph.BroadDiscovery").is_err());
        assert!(validate_strategy_id("graph.broad-discovery").is_ok());
    }

    #[test]
    fn strategy_version_requires_v_prefixed_numeric_parts() {
        assert!(validate_strategy_version("1").is_err());
        assert!(validate_strategy_version("v1.alpha").is_err());
        assert!(validate_strategy_version("v1.2.3").is_ok());
    }
}
