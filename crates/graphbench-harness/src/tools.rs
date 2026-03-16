use crate::runtime::ToolCall;
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolContract {
    pub name: String,
    pub version: String,
    pub input_description: String,
    pub output_description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallTrace {
    pub tool_name: String,
    pub latency_ms: u32,
    pub outcome: String,
    pub input_payload: Value,
    pub output_payload: Value,
}

pub struct ToolInvocationResult {
    pub output: Value,
    pub trace: ToolCallTrace,
    pub canonical_tool_name: String,
    pub mutation_summary: Option<String>,
}

pub struct ToolExecutionResult {
    pub output: Value,
    pub mutation_summary: Option<String>,
}

type ToolHandler = Box<dyn Fn(&Value) -> Result<ToolExecutionResult, AppError>>;
type ToolValidator = Box<dyn Fn(&Value) -> bool>;

struct RegisteredTool {
    contract: ToolContract,
    validate_input: ToolValidator,
    validate_output: ToolValidator,
    handler: ToolHandler,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn register(
        &mut self,
        contract: ToolContract,
        validate_input: impl Fn(&Value) -> bool + 'static,
        validate_output: impl Fn(&Value) -> bool + 'static,
        handler: impl Fn(&Value) -> Result<Value, AppError> + 'static,
    ) {
        self.register_with_result(contract, validate_input, validate_output, move |payload| {
            let output = handler(payload)?;
            Ok(ToolExecutionResult {
                output,
                mutation_summary: None,
            })
        });
    }

    pub fn register_with_result(
        &mut self,
        contract: ToolContract,
        validate_input: impl Fn(&Value) -> bool + 'static,
        validate_output: impl Fn(&Value) -> bool + 'static,
        handler: impl Fn(&Value) -> Result<ToolExecutionResult, AppError> + 'static,
    ) {
        self.tools.insert(
            contract.name.clone(),
            RegisteredTool {
                contract,
                validate_input: Box::new(validate_input),
                validate_output: Box::new(validate_output),
                handler: Box::new(handler),
            },
        );
    }

    pub fn contracts(&self) -> Vec<ToolContract> {
        self.tools
            .values()
            .map(|tool| tool.contract.clone())
            .collect()
    }

    pub fn canonical_tool_name(&self, name: &str) -> Option<String> {
        let requested_name = normalize_tool_name(name);
        self.tools
            .get(requested_name)
            .map(|tool| format!("{}@{}", tool.contract.name, tool.contract.version))
    }

    pub fn invoke(&self, call: &ToolCall) -> Result<ToolInvocationResult, AppError> {
        let requested_name = normalize_tool_name(&call.tool_name);
        let Some(tool) = self.tools.get(requested_name) else {
            return Err(AppError::new(
                ErrorCode::ProviderResponseInvalid,
                format!("unknown tool '{}'", call.tool_name),
                ErrorContext {
                    component: "tool_registry",
                    operation: "invoke",
                },
            ));
        };

        if !(tool.validate_input)(&call.payload) {
            return Err(AppError::new(
                ErrorCode::ProviderResponseInvalid,
                format!(
                    "invalid tool input for '{}': {}",
                    call.tool_name, call.payload
                ),
                ErrorContext {
                    component: "tool_registry",
                    operation: "validate_input",
                },
            ));
        }

        let started = Instant::now();
        let result = (tool.handler)(&call.payload)?;
        if !(tool.validate_output)(&result.output) {
            return Err(AppError::new(
                ErrorCode::ProviderResponseInvalid,
                format!("invalid tool output for '{}'", call.tool_name),
                ErrorContext {
                    component: "tool_registry",
                    operation: "validate_output",
                },
            ));
        }

        Ok(ToolInvocationResult {
            output: result.output.clone(),
            canonical_tool_name: format!("{}@{}", tool.contract.name, tool.contract.version),
            mutation_summary: result.mutation_summary,
            trace: ToolCallTrace {
                tool_name: format!("{}@{}", tool.contract.name, tool.contract.version),
                latency_ms: started.elapsed().as_millis() as u32,
                outcome: "ok".to_owned(),
                input_payload: call.payload.clone(),
                output_payload: result.output,
            },
        })
    }
}

fn normalize_tool_name(name: &str) -> &str {
    name.split_once('@').map_or(name, |(base, _)| base)
}
