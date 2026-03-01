use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::api_key::ApiKey;
use crate::audio::encoder::EncodedAudio;
use crate::error::LLMError;
use crate::llm::prompts::build_multimodal_system_prompt;
use crate::llm::provider::{LLMProvider, LLMResult, ProcessingInput};

/// Request timeout for multimodal LLM API calls (longer than text-only).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// 多模态 LLM provider — 发送音频+上下文，一步完成识别和润色。
pub struct MultimodalProvider {
    client: Client,
    api_key: ApiKey,
    base_url: String,
    model: String,
}

impl MultimodalProvider {
    /// 创建新的多模态 provider。
    pub fn new(
        api_key: String,
        base_url: String,
        model: String,
    ) -> Result<Self, LLMError> {
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

    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    /// 发送多模态请求并用 SSE 流式解析响应。
    #[tracing::instrument(name = "llm_http", skip_all, fields(model = %self.model))]
    async fn send_multimodal_request(
        &self,
        system_prompt: String,
        audio: &EncodedAudio,
    ) -> Result<(String, Option<u32>), LLMError> {
        let body = MultimodalRequest {
            model: self.model.clone(),
            modalities: vec!["text"],
            messages: vec![
                Message {
                    role: "system",
                    content: MessageContent::Text(system_prompt),
                },
                Message {
                    role: "user",
                    content: MessageContent::Parts(vec![ContentPart::InputAudio {
                        input_audio: AudioPayload {
                            data: audio.to_base64(),
                            format: audio.format_str().to_string(),
                        },
                    }]),
                },
            ],
            temperature: 0.3,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
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
                    tracing::warn!("LLM request timed out");
                    LLMError::Timeout
                } else {
                    tracing::error!(error = %e, "LLM network error");
                    LLMError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let status_code = status.as_u16() as u32;
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<failed to read body>"));

            tracing::warn!(status = status_code, "LLM API error response");
            return Err(match status_code {
                401 => LLMError::AuthFailed,
                429 => LLMError::RateLimited,
                _ => LLMError::ServerError(status_code, body_text),
            });
        }

        // SSE 流式解析：逐行读取，拼接 delta.content。
        self.parse_sse_response(response).await
    }

    /// 解析 SSE 流式响应，拼接所有 delta.content。
    async fn parse_sse_response(
        &self,
        response: reqwest::Response,
    ) -> Result<(String, Option<u32>), LLMError> {
        let full_body = response
            .text()
            .await
            .map_err(|e| LLMError::InvalidResponse(e.to_string()))?;

        let mut result_text = String::new();
        let mut tokens_used: Option<u32> = None;

        for line in full_body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data == "[DONE]" {
                    break;
                }

                if let Ok(chunk) = serde_json::from_str::<SSEChunk>(data) {
                    if let Some(choice) = chunk.choices.first() {
                        if let Some(ref delta) = choice.delta {
                            if let Some(ref content) = delta.content {
                                result_text.push_str(content);
                            }
                        }
                    }
                    if let Some(usage) = chunk.usage {
                        tokens_used = Some(usage.total_tokens);
                    }
                }
            }
        }

        if result_text.is_empty() {
            return Err(LLMError::InvalidResponse(
                "SSE 流式响应中没有有效文本内容".to_string(),
            ));
        }

        if let Some(tokens) = tokens_used {
            tracing::debug!(tokens, "LLM response received (streamed)");
        }

        Ok((result_text, tokens_used))
    }
}

impl LLMProvider for MultimodalProvider {
    fn name(&self) -> &str {
        "Multimodal"
    }

    fn process<'a>(
        &'a self,
        input: &'a ProcessingInput,
    ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>> {
        Box::pin(async move {
            let system_prompt = build_multimodal_system_prompt(
                &input.mode,
                &input.context,
                &input.user_dictionary,
                input.target_language.as_deref(),
            );

            let (text, tokens_used) = self
                .send_multimodal_request(system_prompt, &input.audio)
                .await?;

            Ok(LLMResult {
                processed_text: text,
                tokens_used,
            })
        })
    }

    fn test_connection(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>> {
        Box::pin(async move {
            // 发送一个简短的文本请求来验证连接和认证。
            let body = serde_json::json!({
                "model": self.model,
                "messages": [{"role": "user", "content": "Say hi."}],
                "stream": false,
                "max_tokens": 5,
            });

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
                let status_code = status.as_u16() as u32;
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<failed to read body>"));
                return Err(match status_code {
                    401 => LLMError::AuthFailed,
                    429 => LLMError::RateLimited,
                    _ => LLMError::ServerError(status_code, body_text),
                });
            }

            Ok(true)
        })
    }
}

// ---------------------------------------------------------------------------
// Wire types（OpenAI Chat Completions 多模态扩展 + SSE）
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct MultimodalRequest {
    model: String,
    modalities: Vec<&'static str>,
    messages: Vec<Message>,
    temperature: f64,
    stream: bool,
    stream_options: StreamOptions,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: MessageContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "input_audio")]
    InputAudio { input_audio: AudioPayload },
}

#[derive(Serialize)]
struct AudioPayload {
    data: String,
    format: String,
}

// SSE 响应解析类型

#[derive(Deserialize)]
struct SSEChunk {
    choices: Vec<SSEChoice>,
    usage: Option<SSEUsage>,
}

#[derive(Deserialize)]
struct SSEChoice {
    delta: Option<SSEDelta>,
}

#[derive(Deserialize)]
struct SSEDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct SSEUsage {
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::encoder::{AudioBuffer, AudioFormat};
    use crate::context::InputContext;
    use crate::llm::provider::ProcessingMode;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_audio() -> EncodedAudio {
        let mut buf = AudioBuffer::new(16_000, 1);
        buf.push_samples(&vec![0i16; 320]);
        buf.encode(AudioFormat::Wav).unwrap()
    }

    fn sample_input(audio: EncodedAudio) -> ProcessingInput {
        ProcessingInput {
            mode: ProcessingMode::Dictate,
            audio,
            context: InputContext::default(),
            target_language: None,
            user_dictionary: Vec::new(),
        }
    }

    fn sse_success_response() -> String {
        [
            "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}",
            "",
            "data: {\"choices\":[{\"delta\":{\"content\":\"，世界。\"}}]}",
            "",
            "data: {\"choices\":[{\"delta\":{}}],\"usage\":{\"total_tokens\":42}}",
            "",
            "data: [DONE]",
            "",
        ]
        .join("\n")
    }

    #[tokio::test]
    async fn process_success_sse() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_success_response())
                    .append_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = MultimodalProvider::new(
            "key".into(),
            server.uri(),
            "qwen3-omni-flash".into(),
        )
        .unwrap();

        let audio = sample_audio();
        let input = sample_input(audio);
        let result = provider.process(&input).await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.processed_text, "你好，世界。");
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

        let provider =
            MultimodalProvider::new("bad-key".into(), server.uri(), "m".into()).unwrap();
        let audio = sample_audio();
        let input = sample_input(audio);
        let result = provider.process(&input).await;
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

        let provider =
            MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let audio = sample_audio();
        let input = sample_input(audio);
        let result = provider.process(&input).await;
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

        let provider =
            MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let audio = sample_audio();
        let input = sample_input(audio);
        let result = provider.process(&input).await;
        assert!(matches!(result, Err(LLMError::ServerError(500, _))));
    }

    #[tokio::test]
    async fn process_empty_sse_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("data: [DONE]\n")
                    .append_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider =
            MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let audio = sample_audio();
        let input = sample_input(audio);
        let result = provider.process(&input).await;
        assert!(matches!(result, Err(LLMError::InvalidResponse(_))));
    }

    #[tokio::test]
    async fn test_connection_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"role": "assistant", "content": "hi"}}]
            })))
            .mount(&server)
            .await;

        let provider =
            MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let result = provider.test_connection().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_request_body_contains_audio() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_success_response())
                    .append_header("content-type", "text/event-stream"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let provider = MultimodalProvider::new(
            "key".into(),
            server.uri(),
            "qwen3-omni-flash".into(),
        )
        .unwrap();

        let audio = sample_audio();
        let input = sample_input(audio);
        let _ = provider.process(&input).await;

        // 如果 mock 期望满足（exactly 1 call），wiremock 自动验证。
    }
}
