use crate::llm_client::{OpenAiCompatibleClientConfig, OpenAiCompatibleStructuredClient};
use crate::runtime::{ModelClient, ModelInvocation};
use graphbench_core::error::AppError;
use llm::backends::openrouter::OpenRouterConfig;

pub struct OpenRouterClient {
    inner: OpenAiCompatibleStructuredClient<OpenRouterConfig>,
}

impl OpenRouterClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self, AppError> {
        Ok(Self {
            inner: OpenAiCompatibleStructuredClient::new(OpenAiCompatibleClientConfig {
                provider_slug: "openrouter".to_owned(),
                prompt_version: "v1".to_owned(),
                api_key: api_key.into(),
                base_url: None,
                model: model.into(),
                timeout_seconds: 60,
                max_tokens: Some(2_048),
                temperature: None,
                top_p: None,
                top_k: None,
                system_prompt: Some(
                    "You are GraphBench's model runtime. Reply with JSON only. Do not use markdown fences."
                        .to_owned(),
                ),
            })?,
        })
    }
}

impl ModelClient for OpenRouterClient {
    fn respond(&mut self, prompt: &str) -> Result<ModelInvocation, AppError> {
        self.inner.respond(prompt)
    }
}
