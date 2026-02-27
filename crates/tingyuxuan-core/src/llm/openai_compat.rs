use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::api_key::ApiKey;
use crate::error::LLMError;
use crate::llm::prompts::{build_prompt, validate_input};
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
    api_key: ApiKey,
    /// Base URL **without** a trailing slash, e.g. `https://api.openai.com/v1`.
    base_url: String,
    /// Model identifier, e.g. `gpt-4o-mini`, `qwen-turbo`.
    model: String,
}

impl OpenAICompatProvider {
    /// Create a new provider with a shared `reqwest::Client` that enables
    /// connection pooling and keep-alive across requests.
    pub fn new(api_key: String, base_url: String, model: String) -> Result<Self, LLMError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .pool_max_idle_per_host(4)
            .build()
            .map_err(|e| LLMError::HttpClientError(e.to_string()))?;

        Ok(Self {
            client,
            api_key: ApiKey::new(api_key),
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        })
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
            .bearer_auth(self.api_key.expose_secret())
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

impl LLMProvider for OpenAICompatProvider {
    fn name(&self) -> &str {
        "OpenAI-compatible"
    }

    fn process<'a>(
        &'a self,
        input: &'a LLMInput,
    ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>> {
        Box::pin(async move {
            validate_input(input)?;
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
        })
    }

    fn test_connection(&self) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>> {
        Box::pin(async move {
            let messages = vec![ChatMessage {
                role: "user".to_string(),
                content: "Say hi.".to_string(),
            }];

            self.send_request(messages).await?;
            Ok(true)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::{LLMInput, ProcessingMode};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn success_response() -> serde_json::Value {
        serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": "processed text"}}],
            "usage": {"total_tokens": 42}
        })
    }

    fn sample_input() -> LLMInput {
        LLMInput {
            mode: ProcessingMode::Dictate,
            raw_transcript: "hello world".to_string(),
            target_language: None,
            selected_text: None,
            current_app: None,
            user_dictionary: Vec::new(),
        }
    }

    #[tokio::test]
    async fn process_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(success_response()))
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new("key".into(), server.uri(), "gpt-4o-mini".into()).unwrap();
        let result = provider.process(&sample_input()).await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.processed_text, "processed text");
        assert_eq!(r.tokens_used, Some(42));
    }

    #[tokio::test]
    async fn process_auth_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new("bad-key".into(), server.uri(), "m".into()).unwrap();
        let result = provider.process(&sample_input()).await;
        assert!(matches!(result, Err(LLMError::AuthFailed)));
    }

    #[tokio::test]
    async fn process_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Rate limited"))
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let result = provider.process(&sample_input()).await;
        assert!(matches!(result, Err(LLMError::RateLimited)));
    }

    #[tokio::test]
    async fn process_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("ISE"))
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let result = provider.process(&sample_input()).await;
        assert!(matches!(result, Err(LLMError::ServerError(500, _))));
    }

    #[tokio::test]
    async fn process_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let result = provider.process(&sample_input()).await;
        assert!(matches!(result, Err(LLMError::InvalidResponse(_))));
    }

    #[tokio::test]
    async fn test_connection_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(success_response()))
            .mount(&server)
            .await;

        let provider = OpenAICompatProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let result = provider.test_connection().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}
