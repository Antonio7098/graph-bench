use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Configuration,
    Fixture,
    Graph,
    Context,
    Schema,
    Provider,
    Persistence,
    Ui,
}

impl ErrorCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Fixture => "fixture",
            Self::Graph => "graph",
            Self::Context => "context",
            Self::Schema => "schema",
            Self::Provider => "provider",
            Self::Persistence => "persistence",
            Self::Ui => "ui",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    ConfigurationInvalid,
    FixtureManifestInvalid,
    GraphSnapshotMissing,
    ContextReconstructionFailed,
    SchemaValidationFailed,
    ProviderResponseInvalid,
    PersistenceWriteFailed,
    UiContractInvalid,
}

impl ErrorCode {
    #[must_use]
    pub const fn category(self) -> ErrorCategory {
        match self {
            Self::ConfigurationInvalid => ErrorCategory::Configuration,
            Self::FixtureManifestInvalid => ErrorCategory::Fixture,
            Self::GraphSnapshotMissing => ErrorCategory::Graph,
            Self::ContextReconstructionFailed => ErrorCategory::Context,
            Self::SchemaValidationFailed => ErrorCategory::Schema,
            Self::ProviderResponseInvalid => ErrorCategory::Provider,
            Self::PersistenceWriteFailed => ErrorCategory::Persistence,
            Self::UiContractInvalid => ErrorCategory::Ui,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigurationInvalid => "GB-CONFIG-001",
            Self::FixtureManifestInvalid => "GB-FIXTURE-001",
            Self::GraphSnapshotMissing => "GB-GRAPH-001",
            Self::ContextReconstructionFailed => "GB-CONTEXT-001",
            Self::SchemaValidationFailed => "GB-SCHEMA-001",
            Self::ProviderResponseInvalid => "GB-PROVIDER-001",
            Self::PersistenceWriteFailed => "GB-PERSIST-001",
            Self::UiContractInvalid => "GB-UI-001",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    pub component: &'static str,
    pub operation: &'static str,
}

#[derive(Debug)]
pub struct AppError {
    code: ErrorCode,
    message: String,
    context: ErrorContext,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl AppError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>, context: ErrorContext) -> Self {
        Self {
            code,
            message: message.into(),
            context,
            source: None,
        }
    }

    #[must_use]
    pub fn with_source(
        code: ErrorCode,
        message: impl Into<String>,
        context: ErrorContext,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            context,
            source: Some(Box::new(source)),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        self.code.category()
    }

    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        &self.context
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}:{}] {}",
            self.code.as_str(),
            self.context.component,
            self.context.operation,
            self.message
        )
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, ErrorCode, ErrorContext};

    #[test]
    fn error_code_maps_to_stable_category() {
        assert_eq!(
            ErrorCode::SchemaValidationFailed.category().as_str(),
            "schema"
        );
        assert_eq!(ErrorCode::SchemaValidationFailed.as_str(), "GB-SCHEMA-001");
    }

    #[test]
    fn display_includes_code_and_context() {
        let error = AppError::new(
            ErrorCode::ContextReconstructionFailed,
            "context hash mismatch",
            ErrorContext {
                component: "turn_ledger",
                operation: "replay",
            },
        );

        assert_eq!(
            error.to_string(),
            "GB-CONTEXT-001 [turn_ledger:replay] context hash mismatch"
        );
    }
}
