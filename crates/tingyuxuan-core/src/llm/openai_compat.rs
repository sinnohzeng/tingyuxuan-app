use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::LLMError;
use crate::llm::prompts::build_prompt;
use crate::llm::provider::{LLMInput, LLMProvider, LLMResult};

/// Request timeout for LLM API calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// An LLM provider that speaks the OpenAI-compatible chat completions API.
///
/// Works with OpenAI, DashScope, Volcengine (Doubao), and any other provider
/// that implements the `/chat/completions` endpoint.
pub struct OpenAICompatProvider {
    /// Shared HTTP client with connection pooling and keep-alive.
    client: Client,
    /// Bearer token for the `Authorization` header.
    api_key: String,
    /// Base URL **without** a trailing slash, e.g. `https://api.openai.com/v1`.
    base_url: String,
    /// Model identifier, e.g. `gpt-4o-mini`, `qwen-turbo`.
    model: String,
}

impl OpenAICompatProvider {
    /// Create a new provider with a shared `reqwest::Client` that enables
    /// connection pooling and keep-alive across requests.
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .pool_max_idle_per_host(4)
            .build()
            .expect("failed to build reqwest client");

        Self {
            client,
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        }
    }

    /// Build the full endpoint URL for chat completions.
    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    /// Send a chat completion request and map transport / HTTP errors into
    /// [`LLMError`] variants.
    async fn send_request(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatCompletionResponse, LLMError> {
        let body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature: 0.3,
        };

        let response = self
            .client
            .post(self.completions_url())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout
                } else {
                    LLMError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let status_code = status.as_u16();
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<failed to read body>"));

            return Err(match status_code {
                401 => LLMError::AuthFailed,
                429 => LLMError::RateLimited,
                500..=599 => LLMError::ServerError(status_code, body_text),
                _ => LLMError::ServerError(status_code, body_text),
            });
        }

        response
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|e| LLMError::InvalidResponse(e.to_string()))
    }
}

#[async_trait]
impl LLMProvider for OpenAICompatProvider {
    fn name(&self) -> &str {
        "OpenAI-compatible"
    }

    async fn process(&self, input: &LLMInput) -> Result<LLMResult, LLMError> {
        let (system_msg, user_msg) = build_prompt(&input.mode, input);

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_msg,
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_msg,
            },
        ];

        let resp = self.send_request(messages).await?;

        let text = resp
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone())
            .ok_or_else(|| LLMError::InvalidResponse("no choices in response".to_string()))?;

        let tokens_used = resp.usage.map(|u| u.total_tokens);

        Ok(LLMResult {
            processed_text: text,
            tokens_used,
        })
    }

    async fn test_connection(&self) -> Result<bool, LLMError> {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Say hi.".to_string(),
        }];

        self.send_request(messages).await?;
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// OpenAI chat completions wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    total_tokens: u32,
}
