use crate::runtime::{HarnessModelResponse, ModelClient, ModelInvocation};
use graphbench_core::error::{AppError, ErrorCode, ErrorContext};
use llm::chat::{ChatMessage, ChatRole, MessageType, StructuredOutputFormat};
use llm::providers::openai_compatible::{
    OpenAIChatRequest, OpenAIChatResponse, OpenAICompatibleProvider, OpenAIProviderConfig,
    OpenAIResponseFormat,
};
use serde_json::{Value, json};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClientConfig {
    pub provider_slug: String,
    pub prompt_version: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub timeout_seconds: u64,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub system_prompt: Option<String>,
}

pub struct OpenAiCompatibleStructuredClient<T: OpenAIProviderConfig> {
    provider: OpenAICompatibleProvider<T>,
    config: OpenAiCompatibleClientConfig,
    runtime: Runtime,
}

impl<T: OpenAIProviderConfig> OpenAiCompatibleStructuredClient<T> {
    pub fn new(config: OpenAiCompatibleClientConfig) -> Result<Self, AppError> {
        let provider = OpenAICompatibleProvider::<T>::new(
            config.api_key.clone(),
            config.base_url.clone(),
            Some(config.model.clone()),
            config.max_tokens,
            config.temperature,
            Some(config.timeout_seconds),
            config.system_prompt.clone(),
            config.top_p,
            config.top_k,
            None,
            None,
            None,
            Some(harness_response_schema()),
            None,
            None,
            Some(false),
            Some(true),
            None,
            None,
        );
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::ProviderResponseInvalid,
                    "failed to construct llm runtime",
                    ErrorContext {
                        component: "llm_client",
                        operation: "new_runtime",
                    },
                    source,
                )
            })?;

        Ok(Self {
            provider,
            config,
            runtime,
        })
    }

    fn provider_request(
        &self,
        prompt: &str,
    ) -> Result<(Value, Value, OpenAIChatResponse, Option<String>), AppError> {
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            message_type: MessageType::Text,
            content: prompt.to_owned(),
        }];
        let request = OpenAIChatRequest {
            model: self.provider.model(),
            messages: self.provider.prepare_messages(&messages),
            max_tokens: self.provider.max_tokens(),
            temperature: self.provider.temperature(),
            stream: false,
            top_p: self.provider.top_p(),
            top_k: self.provider.top_k(),
            tools: None,
            tool_choice: None,
            reasoning_effort: None,
            response_format: Some(OpenAIResponseFormat::from(harness_response_schema())),
            stream_options: None,
            parallel_tool_calls: Some(false),
            extra_body: self.provider.extra_body().clone(),
        };
        let request_json = serde_json::to_value(&request).map_err(|source| {
            AppError::with_source(
                ErrorCode::ProviderResponseInvalid,
                "failed to serialize llm provider request",
                ErrorContext {
                    component: "llm_client",
                    operation: "serialize_request",
                },
                source,
            )
        })?;

        let url = self
            .provider
            .base_url()
            .join(T::CHAT_ENDPOINT)
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::ProviderResponseInvalid,
                    "failed to build provider endpoint URL",
                    ErrorContext {
                        component: "llm_client",
                        operation: "build_url",
                    },
                    source,
                )
            })?;

        let response = self.runtime.block_on(async {
            let mut request_builder = self
                .provider
                .client()
                .post(url)
                .bearer_auth(self.provider.api_key())
                .json(&request);
            if let Some(headers) = T::custom_headers() {
                for (key, value) in headers {
                    request_builder = request_builder.header(key, value);
                }
            }
            if let Some(timeout) = self.provider.timeout_seconds() {
                request_builder = request_builder.timeout(std::time::Duration::from_secs(timeout));
            }
            request_builder.send().await
        });

        let response = response.map_err(|source| {
            AppError::with_source(
                ErrorCode::ProviderResponseInvalid,
                "failed to send llm provider request",
                ErrorContext {
                    component: "llm_client",
                    operation: "send_request",
                },
                source,
            )
        })?;

        let status = response.status();
        let headers = response.headers().clone();
        let mut provider_request_id = headers
            .get("x-request-id")
            .or_else(|| headers.get("request-id"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body_text = self.runtime.block_on(response.text()).map_err(|source| {
            AppError::with_source(
                ErrorCode::ProviderResponseInvalid,
                "failed to read llm provider response body",
                ErrorContext {
                    component: "llm_client",
                    operation: "read_response",
                },
                source,
            )
        })?;

        if !status.is_success() {
            return Err(AppError::new(
                ErrorCode::ProviderResponseInvalid,
                format!("provider returned HTTP {}: {}", status.as_u16(), body_text),
                ErrorContext {
                    component: "llm_client",
                    operation: "validate_status",
                },
            ));
        }

        let response_json: Value = serde_json::from_str(&body_text).map_err(|source| {
            AppError::with_source(
                ErrorCode::ProviderResponseInvalid,
                "failed to parse raw llm provider response envelope",
                ErrorContext {
                    component: "llm_client",
                    operation: "parse_response_envelope",
                },
                source,
            )
        })?;
        let parsed_response: OpenAIChatResponse = serde_json::from_value(response_json.clone())
            .map_err(|source| {
                AppError::with_source(
                    ErrorCode::ProviderResponseInvalid,
                    "failed to decode llm provider response envelope",
                    ErrorContext {
                        component: "llm_client",
                        operation: "decode_response_envelope",
                    },
                    source,
                )
            })?;
        if provider_request_id.is_none() {
            provider_request_id = response_json
                .get("id")
                .and_then(|value| value.as_str())
                .map(str::to_owned);
        }

        let raw_response = json!({
            "status": status.as_u16(),
            "headers": headers_to_json(&headers),
            "envelope": response_json,
        });

        Ok((
            json!({
                "provider": self.config.provider_slug,
                "request": request_json,
            }),
            raw_response,
            parsed_response,
            provider_request_id,
        ))
    }
}

impl<T: OpenAIProviderConfig> ModelClient for OpenAiCompatibleStructuredClient<T> {
    fn respond(&mut self, prompt: &str) -> Result<ModelInvocation, AppError> {
        let (raw_request, raw_response, parsed_response, provider_request_id) =
            self.provider_request(prompt)?;
        let response_text = parsed_response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::ProviderResponseInvalid,
                    "llm provider returned no text payload for structured response",
                    ErrorContext {
                        component: "llm_client",
                        operation: "extract_text",
                    },
                )
            })?;

        let json_payload = extract_json_payload(response_text)?;
        let mut parsed: HarnessModelResponse =
            serde_json::from_value(json_payload).map_err(|source| {
                AppError::with_source(
                    ErrorCode::ProviderResponseInvalid,
                    format!(
                        "failed to parse llm response into HarnessModelResponse: {} | payload={}",
                        source, response_text
                    ),
                    ErrorContext {
                        component: "llm_client",
                        operation: "parse_model_response",
                    },
                    std::io::Error::other("llm response schema mismatch"),
                )
            })?;
        if parsed.tool_call.is_some() {
            parsed.kind = crate::runtime::ModelResponseKind::ToolCall;
        }
        if parsed.assistant_message.trim().is_empty() {
            parsed.assistant_message = match parsed.kind {
                crate::runtime::ModelResponseKind::ToolCall => "Tool call requested.".to_owned(),
                crate::runtime::ModelResponseKind::Complete => "Run completed.".to_owned(),
                crate::runtime::ModelResponseKind::Think => "Thinking.".to_owned(),
            };
        }
        parsed.prompt_version = self.config.prompt_version.clone();
        parsed.provider = self.config.provider_slug.clone();
        parsed.model_slug = self.config.model.clone();

        Ok(ModelInvocation {
            response: parsed,
            raw_request,
            raw_response,
            provider_request_id,
        })
    }
}

fn harness_response_schema() -> StructuredOutputFormat {
    StructuredOutputFormat {
        name: "graphbench_harness_response".to_owned(),
        description: Some("Structured GraphBench harness response".to_owned()),
        schema: Some(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["think", "tool_call", "complete"]
                },
                "assistant_message": { "type": "string" },
                "tool_call": {
                    "type": ["object", "null"],
                    "additionalProperties": false,
                    "properties": {
                        "tool_name": { "type": "string" },
                        "payload": { "type": "object" }
                    },
                    "required": ["tool_name", "payload"]
                },
                "acquired_fact_ids": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "readiness_state": {
                    "type": "string",
                    "enum": [
                        "not_ready",
                        "evidence_visible",
                        "evidence_acquired",
                        "ready_to_edit"
                    ]
                }
            },
            "required": [
                "kind",
                "assistant_message",
                "acquired_fact_ids",
                "readiness_state"
            ]
        })),
        strict: Some(true),
    }
}

fn extract_json_payload(content: &str) -> Result<Value, AppError> {
    if let Ok(value) = serde_json::from_str::<Value>(content) {
        return Ok(value);
    }

    let trimmed = content.trim();
    let trimmed = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    let trimmed = trimmed.strip_suffix("```").unwrap_or(trimmed).trim();
    if let Some(candidate) = first_balanced_json_object(trimmed) {
        if let Ok(value) = serde_json::from_str::<Value>(&candidate) {
            return Ok(value);
        }
    }
    serde_json::from_str(trimmed).map_err(|source| {
        AppError::with_source(
            ErrorCode::ProviderResponseInvalid,
            format!("provider content was not valid JSON | payload={content}"),
            ErrorContext {
                component: "llm_client",
                operation: "extract_json_payload",
            },
            source,
        )
    })
}

fn first_balanced_json_object(content: &str) -> Option<String> {
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in content.char_indices() {
        if start.is_none() {
            if ch == '{' {
                start = Some(index);
                depth = 1;
            }
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let start_index = start.expect("start should be set when depth is tracked");
                    return Some(content[start_index..=index].to_owned());
                }
            }
            _ => {}
        }
    }

    None
}

fn headers_to_json(headers: &reqwest::header::HeaderMap) -> Value {
    let pairs = headers
        .iter()
        .map(|(key, value)| {
            (
                key.as_str().to_owned(),
                Value::String(value.to_str().unwrap_or("<non-utf8>").to_owned()),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    Value::Object(pairs)
}

#[cfg(test)]
mod tests {
    use super::extract_json_payload;

    #[test]
    fn extract_json_payload_recovers_first_balanced_object() {
        let content =
            "{\n  \"kind\": \"complete\",\n  \"prompt_version\": \"v1\"\n}\n\ntrailing noise";
        let parsed = extract_json_payload(content).expect("json payload");
        assert_eq!(parsed["kind"], "complete");
        assert_eq!(parsed["prompt_version"], "v1");
    }
}
