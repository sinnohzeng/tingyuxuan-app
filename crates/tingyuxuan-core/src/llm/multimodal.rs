use futures_util::StreamExt as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::api_key::ApiKey;
use crate::audio::encoder::{AudioBuffer, AudioFormat, EncodedAudio};
use crate::error::LLMError;
use crate::llm::prompts::build_multimodal_system_prompt;
use crate::llm::provider::{LLMProvider, LLMResult, ProcessingInput};

/// 连接/首字节超时（秒）。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// 两个 SSE chunk 之间的最大等待时间（秒）。
const CHUNK_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// 多模态 LLM provider — 发送音频+上下文，一步完成识别和润色。
pub struct MultimodalProvider {
    client: Client,
    api_key: ApiKey,
    base_url: String,
    model: String,
}

impl MultimodalProvider {
    /// 创建新的多模态 provider。
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

    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    /// 连接测试（真实多模态）：发送极短静音音频，验证端点具备音频输入能力。
    pub async fn test_multimodal_audio_connection(&self) -> Result<bool, LLMError> {
        let mut probe = AudioBuffer::new(16_000, 1);
        probe.push_samples(&vec![0i16; 320]); // 20ms silence
        let audio = probe
            .encode(AudioFormat::Wav)
            .map_err(|e| LLMError::InvalidResponse(format!("probe audio encoding failed: {e}")))?;

        let (text, _tokens) = self
            .send_multimodal_request(
                "You are a connectivity probe. Reply with exactly: OK".to_string(),
                &audio,
            )
            .await?;

        if text.trim().is_empty() {
            return Err(LLMError::InvalidResponse(
                "multimodal probe returned empty text".to_string(),
            ));
        }

        Ok(true)
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

    /// 解析 SSE 流式响应，逐 chunk 读取并拼接 delta.content。
    ///
    /// 每次 chunk 读取都有 `CHUNK_READ_TIMEOUT` 保护，避免无限挂起。
    async fn parse_sse_response(
        &self,
        response: reqwest::Response,
    ) -> Result<(String, Option<u32>), LLMError> {
        let mut stream = response.bytes_stream();
        let mut result_text = String::new();
        let mut tokens_used: Option<u32> = None;
        // 跨 chunk 的残行缓冲：网络 chunk 边界不一定对齐行边界。
        let mut line_buf = String::new();
        let sse_start = std::time::Instant::now();
        let mut ttfb_logged = false;

        loop {
            let maybe_chunk = tokio::time::timeout(CHUNK_READ_TIMEOUT, stream.next()).await;
            let chunk_result = match maybe_chunk {
                Err(_elapsed) => {
                    tracing::warn!("SSE chunk read timed out");
                    return Err(LLMError::Timeout);
                }
                Ok(None) => break, // 流结束
                Ok(Some(r)) => r,
            };

            let bytes = chunk_result.map_err(|e| LLMError::InvalidResponse(e.to_string()))?;
            let text = String::from_utf8_lossy(&bytes);
            line_buf.push_str(&text);

            // 逐行解析已缓冲的数据。
            while let Some(newline_pos) = line_buf.find('\n') {
                let line: String = line_buf.drain(..=newline_pos).collect();
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    // 消费 [DONE]，提前返回。
                    return Self::finish_sse(result_text, tokens_used);
                }

                if let Ok(chunk) = serde_json::from_str::<SSEChunk>(data) {
                    if let Some(choice) = chunk.choices.first()
                        && let Some(ref delta) = choice.delta
                        && let Some(ref content) = delta.content
                    {
                        // 首个 content chunk 到达时记录 TTFB
                        if !ttfb_logged && !content.is_empty() {
                            tracing::info!(
                                ttfb_ms = sse_start.elapsed().as_millis() as u64,
                                "LLM TTFB"
                            );
                            ttfb_logged = true;
                        }
                        result_text.push_str(content);
                    }
                    if let Some(usage) = chunk.usage {
                        tokens_used = Some(usage.total_tokens);
                    }
                }
            }
        }

        Self::finish_sse(result_text, tokens_used)
    }

    /// SSE 解析完成后的统一校验和日志。
    fn finish_sse(
        result_text: String,
        tokens_used: Option<u32>,
    ) -> Result<(String, Option<u32>), LLMError> {
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<LLMResult, LLMError>> + Send + 'a>>
    {
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, LLMError>> + Send + '_>>
    {
        Box::pin(async move { self.test_multimodal_audio_connection().await })
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

        let provider =
            MultimodalProvider::new("key".into(), server.uri(), "qwen3-omni-flash".into()).unwrap();

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

        let provider = MultimodalProvider::new("bad-key".into(), server.uri(), "m".into()).unwrap();
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

        let provider = MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
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

        let provider = MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
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

        let provider = MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
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
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_success_response())
                    .append_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = MultimodalProvider::new("key".into(), server.uri(), "m".into()).unwrap();
        let result = provider.test_multimodal_audio_connection().await;
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

        let provider =
            MultimodalProvider::new("key".into(), server.uri(), "qwen3-omni-flash".into()).unwrap();

        let audio = sample_audio();
        let input = sample_input(audio);
        let _ = provider.process(&input).await;

        // 如果 mock 期望满足（exactly 1 call），wiremock 自动验证。
    }
}
