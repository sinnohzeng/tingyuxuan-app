use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

use crate::error::STTError;
use crate::stt::provider::{STTOptions, STTProvider, STTResult};

/// Alibaba Cloud DashScope Qwen-ASR speech-to-text provider.
pub struct DashScopeASRProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

// ---- Request types ----

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: Vec<ContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "input_audio")]
    InputAudio { input_audio: AudioData },
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Serialize)]
struct AudioData {
    data: String,
    format: String,
}

// ---- Response types ----

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

// ---- Simple text request/response for test_connection ----

#[derive(Debug, Serialize)]
struct SimpleTextRequest {
    model: String,
    messages: Vec<SimpleMessage>,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct SimpleMessage {
    role: String,
    content: String,
}

impl DashScopeASRProvider {
    /// Create a new DashScopeASRProvider.
    ///
    /// - `api_key`: The API key for authentication.
    /// - `base_url`: The base URL of the API (defaults to `https://dashscope.aliyuncs.com/compatible-mode/v1`).
    /// - `model`: The model to use (defaults to `qwen2-audio-instruct`).
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            api_key,
            base_url: base_url.unwrap_or_else(|| {
                "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()
            }),
            model: model.unwrap_or_else(|| "qwen2-audio-instruct".to_string()),
        }
    }

    /// Map an HTTP status code and body to an appropriate STTError.
    fn map_http_error(status: reqwest::StatusCode, body: &str) -> STTError {
        match status.as_u16() {
            401 => STTError::AuthFailed,
            429 => STTError::RateLimited,
            code if code >= 500 => STTError::ServerError(code, body.to_string()),
            code => STTError::ServerError(code, body.to_string()),
        }
    }
}

#[async_trait]
impl STTProvider for DashScopeASRProvider {
    fn name(&self) -> &str {
        "dashscope_asr"
    }

    async fn transcribe(
        &self,
        audio_path: &Path,
        options: &STTOptions,
    ) -> Result<STTResult, STTError> {
        // Read the audio file and base64-encode it
        let file_bytes = tokio::fs::read(audio_path)
            .await
            .map_err(|e| STTError::NetworkError(format!("Failed to read audio file: {}", e)))?;

        let encoded = base64::engine::general_purpose::STANDARD.encode(&file_bytes);

        // Build the transcription prompt
        let prompt_text = if let Some(ref prompt) = options.prompt {
            format!(
                "Please transcribe this audio accurately. Vocabulary hints: {}. Output only the transcription text, nothing else.",
                prompt
            )
        } else {
            "Please transcribe this audio accurately. Output only the transcription text, nothing else.".to_string()
        };

        // Build the request body
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![
                    ContentPart::InputAudio {
                        input_audio: AudioData {
                            data: encoded,
                            format: "wav".to_string(),
                        },
                    },
                    ContentPart::Text { text: prompt_text },
                ],
            }],
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    STTError::Timeout(15)
                } else {
                    STTError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("(failed to read response body)"));
            return Err(Self::map_http_error(status, &body));
        }

        let body = response
            .text()
            .await
            .map_err(|e| STTError::InvalidResponse(format!("Failed to read response: {}", e)))?;

        let chat_resp: ChatCompletionResponse = serde_json::from_str(&body)
            .map_err(|e| STTError::InvalidResponse(format!("Failed to parse JSON: {}", e)))?;

        let text = chat_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| {
                STTError::InvalidResponse("No choices in response".to_string())
            })?;

        let language = options
            .language
            .clone()
            .unwrap_or_else(|| "auto".to_string());

        Ok(STTResult {
            text,
            language,
            duration_seconds: 0.0,
        })
    }

    async fn test_connection(&self) -> Result<bool, STTError> {
        // Send a simple text-only chat completion to verify the API key.
        let request = SimpleTextRequest {
            model: self.model.clone(),
            messages: vec![SimpleMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            }],
            max_tokens: 1,
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    STTError::Timeout(15)
                } else {
                    STTError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("(failed to read response body)"));
            return Err(Self::map_http_error(status, &body));
        }

        Ok(true)
    }
}
