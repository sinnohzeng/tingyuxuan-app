use thiserror::Error;

/// User-facing action to show when an error occurs.
#[derive(Debug, Clone, serde::Serialize)]
pub enum UserAction {
    /// STT network failure → [Retry]
    Retry,
    /// LLM failure (STT succeeded) → [Insert Raw Transcript] [Retry Processing]
    InsertRawOrRetry,
    /// 401 auth failure → [Go to Settings]
    CheckApiKey,
    /// 429 rate limit → auto-delay retry
    WaitAndRetry,
    /// Microphone unavailable → [Go to System Settings]
    CheckMicrophone,
}

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("No audio input device found")]
    NoInputDevice,
    #[error("Microphone permission denied")]
    PermissionDenied,
    #[error("Microphone is in use by another application")]
    DeviceBusy,
    #[error("Audio stream error: {0}")]
    StreamError(String),
    #[error("Audio metadata error: {0}")]
    MetadataError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Not recording")]
    NotRecording,
    #[error("Already recording")]
    AlreadyRecording,
}

#[derive(Error, Debug, Clone)]
pub enum STTError {
    #[error("Network timeout (>{0}s)")]
    Timeout(u64),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Authentication failed (HTTP 401): check your API key")]
    AuthFailed,
    #[error("Rate limited (HTTP 429): try again later")]
    RateLimited,
    #[error("Server error ({0}): {1}")]
    ServerError(u32, String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Provider not configured")]
    NotConfigured,
    #[error("Unsupported audio format")]
    UnsupportedFormat,
    #[error("HTTP client initialization failed: {0}")]
    HttpClientError(String),
    #[error("Input too large: {0}")]
    InputTooLarge(String),
}

#[derive(Error, Debug)]
pub enum LLMError {
    #[error("Network timeout")]
    Timeout,
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Authentication failed (HTTP 401): check your API key")]
    AuthFailed,
    #[error("Rate limited (HTTP 429): try again later")]
    RateLimited,
    #[error("Server error ({0}): {1}")]
    ServerError(u32, String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Provider not configured")]
    NotConfigured,
    #[error("HTTP client initialization failed: {0}")]
    HttpClientError(String),
    #[error("Input too large: {0}")]
    InputTooLarge(String),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Config directory not found")]
    NoDirFound,
}

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),
    #[error("STT error: {0}")]
    Stt(#[from] STTError),
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),
    #[error("Pipeline cancelled by user")]
    Cancelled,
    #[error("Pipeline is busy")]
    Busy,
}

#[derive(Error, Debug)]
pub enum HistoryError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl STTError {
    /// Maps this error to a user-facing action.
    pub fn user_action(&self) -> UserAction {
        match self {
            STTError::AuthFailed => UserAction::CheckApiKey,
            STTError::RateLimited => UserAction::WaitAndRetry,
            _ => UserAction::Retry,
        }
    }
}

impl LLMError {
    /// Maps this error to a user-facing action.
    pub fn user_action(&self) -> UserAction {
        match self {
            LLMError::AuthFailed => UserAction::CheckApiKey,
            LLMError::RateLimited => UserAction::WaitAndRetry,
            _ => UserAction::InsertRawOrRetry,
        }
    }
}

impl PipelineError {
    pub fn user_action(&self) -> UserAction {
        match self {
            PipelineError::Audio(AudioError::NoInputDevice | AudioError::PermissionDenied) => {
                UserAction::CheckMicrophone
            }
            PipelineError::Stt(e) => e.user_action(),
            PipelineError::Llm(e) => e.user_action(),
            PipelineError::Busy => UserAction::WaitAndRetry,
            _ => UserAction::Retry,
        }
    }
}

// ---------------------------------------------------------------------------
// StructuredError — 平台无关的结构化错误，供 JNI/Tauri 共用
// ---------------------------------------------------------------------------

/// 平台无关的结构化错误，供 JNI 和 Tauri 层统一消费。
///
/// 将 `PipelineError` 的内部分类映射为前端/Kotlin 可直接使用的
/// error_code + message + user_action 三元组。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StructuredError {
    pub error_code: String,
    pub message: String,
    pub user_action: UserAction,
}

impl From<&PipelineError> for StructuredError {
    fn from(e: &PipelineError) -> Self {
        match e {
            PipelineError::Stt(stt_err) => {
                let error_code = match stt_err {
                    STTError::AuthFailed => "stt_auth_failed",
                    STTError::Timeout(_) => "timeout",
                    STTError::NetworkError(_) => "network_error",
                    STTError::NotConfigured => "not_configured",
                    STTError::RateLimited => "rate_limited",
                    _ => "stt_error",
                };
                StructuredError {
                    error_code: error_code.to_string(),
                    message: e.to_string(),
                    user_action: stt_err.user_action(),
                }
            }
            PipelineError::Llm(llm_err) => {
                let error_code = match llm_err {
                    LLMError::AuthFailed => "llm_auth_failed",
                    LLMError::Timeout => "timeout",
                    LLMError::NetworkError(_) => "network_error",
                    LLMError::NotConfigured => "not_configured",
                    LLMError::RateLimited => "rate_limited",
                    _ => "llm_error",
                };
                StructuredError {
                    error_code: error_code.to_string(),
                    message: e.to_string(),
                    user_action: llm_err.user_action(),
                }
            }
            PipelineError::Cancelled => StructuredError {
                error_code: "cancelled".to_string(),
                message: "Processing cancelled".to_string(),
                user_action: UserAction::Retry,
            },
            PipelineError::Busy => StructuredError {
                error_code: "busy".to_string(),
                message: "Pipeline is busy".to_string(),
                user_action: UserAction::WaitAndRetry,
            },
            PipelineError::Audio(audio_err) => StructuredError {
                error_code: "audio_error".to_string(),
                message: audio_err.to_string(),
                user_action: e.user_action(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_error_from_stt_auth() {
        let e = PipelineError::Stt(STTError::AuthFailed);
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "stt_auth_failed");
        assert!(matches!(se.user_action, UserAction::CheckApiKey));
    }

    #[test]
    fn test_structured_error_from_stt_timeout() {
        let e = PipelineError::Stt(STTError::Timeout(30));
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "timeout");
        assert!(matches!(se.user_action, UserAction::Retry));
    }

    #[test]
    fn test_structured_error_from_llm_timeout() {
        let e = PipelineError::Llm(LLMError::Timeout);
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "timeout");
        assert!(matches!(se.user_action, UserAction::InsertRawOrRetry));
    }

    #[test]
    fn test_structured_error_from_cancelled() {
        let e = PipelineError::Cancelled;
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "cancelled");
    }

    #[test]
    fn test_structured_error_from_busy() {
        let e = PipelineError::Busy;
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "busy");
        assert!(matches!(se.user_action, UserAction::WaitAndRetry));
    }

    #[test]
    fn test_structured_error_from_audio() {
        let e = PipelineError::Audio(AudioError::NoInputDevice);
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "audio_error");
        assert!(matches!(se.user_action, UserAction::CheckMicrophone));
    }

    #[test]
    fn test_structured_error_json_serializable() {
        let e = PipelineError::Stt(STTError::AuthFailed);
        let se = StructuredError::from(&e);
        let json = serde_json::to_string(&se).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error_code"], "stt_auth_failed");
        assert_eq!(parsed["user_action"], "CheckApiKey");
    }

    #[test]
    fn test_structured_error_from_llm_rate_limited() {
        let e = PipelineError::Llm(LLMError::RateLimited);
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "rate_limited");
        assert!(matches!(se.user_action, UserAction::WaitAndRetry));
    }

    #[test]
    fn test_structured_error_from_stt_network() {
        let e = PipelineError::Stt(STTError::NetworkError("connection reset".to_string()));
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "network_error");
        assert!(matches!(se.user_action, UserAction::Retry));
        assert!(se.message.contains("connection reset"));
    }

    #[test]
    fn test_structured_error_from_not_configured() {
        let e = PipelineError::Stt(STTError::NotConfigured);
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "not_configured");

        let e = PipelineError::Llm(LLMError::NotConfigured);
        let se = StructuredError::from(&e);
        assert_eq!(se.error_code, "not_configured");
    }
}
