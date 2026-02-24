use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

use crate::error::STTError;
use crate::stt::provider::{STTOptions, STTProvider, STTResult};

/// OpenAI Whisper API compatible speech-to-text provider.
pub struct WhisperProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

/// JSON response from the Whisper transcription endpoint.
#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
}

impl WhisperProvider {
    /// Create a new WhisperProvider.
    ///
    /// - `api_key`: The API key for authentication.
    /// - `base_url`: The base URL of the API (defaults to `https://api.openai.com/v1`).
    /// - `model`: The model to use (defaults to `whisper-1`).
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            model: model.unwrap_or_else(|| "whisper-1".to_string()),
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
impl STTProvider for WhisperProvider {
    fn name(&self) -> &str {
        "whisper"
    }

    async fn transcribe(
        &self,
        audio_path: &Path,
        options: &STTOptions,
    ) -> Result<STTResult, STTError> {
        // Read the audio file
        let file_bytes = tokio::fs::read(audio_path)
            .await
            .map_err(|e| STTError::NetworkError(format!("Failed to read audio file: {}", e)))?;

        let file_name = audio_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav")
            .to_string();

        // Build multipart form
        let file_part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("audio/wav")
            .map_err(|e| STTError::NetworkError(format!("Failed to set MIME type: {}", e)))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "json");

        if let Some(ref lang) = options.language {
            if lang != "auto" {
                form = form.text("language", lang.clone());
            }
        }

        if let Some(ref prompt) = options.prompt {
            form = form.text("prompt", prompt.clone());
        }

        let url = format!("{}/audio/transcriptions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
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

        let whisper_resp: WhisperResponse = serde_json::from_str(&body)
            .map_err(|e| STTError::InvalidResponse(format!("Failed to parse JSON: {}", e)))?;

        let language = options
            .language
            .clone()
            .unwrap_or_else(|| "auto".to_string());

        Ok(STTResult {
            text: whisper_resp.text,
            language,
            duration_seconds: 0.0, // Whisper API does not return duration in the basic response
        })
    }

    async fn test_connection(&self) -> Result<bool, STTError> {
        let url = format!("{}/models", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Create a minimal valid WAV file for testing.
    fn dummy_audio_file() -> NamedTempFile {
        let f = NamedTempFile::new().unwrap();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(f.path(), spec).unwrap();
        for _ in 0..160 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        f
    }

    fn default_opts() -> STTOptions {
        STTOptions {
            language: None,
            prompt: None,
        }
    }

    #[tokio::test]
    async fn transcribe_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/transcriptions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"text": "Hello world"})),
            )
            .mount(&server)
            .await;

        let provider = WhisperProvider::new("test-key".into(), Some(server.uri()), None);
        let audio = dummy_audio_file();
        let result = provider.transcribe(audio.path(), &default_opts()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().text, "Hello world");
    }

    #[tokio::test]
    async fn transcribe_auth_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let provider = WhisperProvider::new("bad-key".into(), Some(server.uri()), None);
        let audio = dummy_audio_file();
        let result = provider.transcribe(audio.path(), &default_opts()).await;
        assert!(matches!(result, Err(STTError::AuthFailed)));
    }

    #[tokio::test]
    async fn transcribe_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Rate limited"))
            .mount(&server)
            .await;

        let provider = WhisperProvider::new("key".into(), Some(server.uri()), None);
        let audio = dummy_audio_file();
        let result = provider.transcribe(audio.path(), &default_opts()).await;
        assert!(matches!(result, Err(STTError::RateLimited)));
    }

    #[tokio::test]
    async fn transcribe_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let provider = WhisperProvider::new("key".into(), Some(server.uri()), None);
        let audio = dummy_audio_file();
        let result = provider.transcribe(audio.path(), &default_opts()).await;
        assert!(matches!(result, Err(STTError::ServerError(500, _))));
    }

    #[tokio::test]
    async fn transcribe_invalid_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
            .mount(&server)
            .await;

        let provider = WhisperProvider::new("key".into(), Some(server.uri()), None);
        let audio = dummy_audio_file();
        let result = provider.transcribe(audio.path(), &default_opts()).await;
        assert!(matches!(result, Err(STTError::InvalidResponse(_))));
    }

    #[tokio::test]
    async fn test_connection_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&server)
            .await;

        let provider = WhisperProvider::new("key".into(), Some(server.uri()), None);
        let result = provider.test_connection().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}
