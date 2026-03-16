#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogField {
    RunId,
    TaskId,
    TurnIndex,
    StrategyId,
    FixtureId,
    Component,
    ErrorCode,
}

impl LogField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RunId => "run_id",
            Self::TaskId => "task_id",
            Self::TurnIndex => "turn_index",
            Self::StrategyId => "strategy_id",
            Self::FixtureId => "fixture_id",
            Self::Component => "component",
            Self::ErrorCode => "error_code",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogEvent {
    RunStarted,
    RunFinished,
    TurnStarted,
    TurnFinished,
    ValidationFailed,
    PersistenceWritten,
}

impl LogEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RunStarted => "run.started",
            Self::RunFinished => "run.finished",
            Self::TurnStarted => "turn.started",
            Self::TurnFinished => "turn.finished",
            Self::ValidationFailed => "validation.failed",
            Self::PersistenceWritten => "persistence.written",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LogEvent, LogField};

    #[test]
    fn fields_are_stable_and_machine_readable() {
        assert_eq!(LogField::RunId.as_str(), "run_id");
        assert_eq!(LogField::ErrorCode.as_str(), "error_code");
    }

    #[test]
    fn events_follow_namespaced_convention() {
        assert_eq!(LogEvent::RunStarted.as_str(), "run.started");
        assert_eq!(LogEvent::ValidationFailed.as_str(), "validation.failed");
    }
}
