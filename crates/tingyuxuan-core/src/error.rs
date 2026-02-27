use thiserror::Error;

/// User-facing action to show when an error occurs.
#[derive(Debug, Clone, serde::Serialize)]
pub enum UserAction {
    /// STT network failure → [Retry] [Queue for Later]
    RetryOrQueue,
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
    #[error("WAV write error: {0}")]
    WavWriteError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Not recording")]
    NotRecording,
    #[error("Already recording")]
    AlreadyRecording,
}

#[derive(Error, Debug)]
pub enum STTError {
    #[error("Network timeout (>{0}s)")]
    Timeout(u64),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Authentication failed (HTTP 401): check your API key")]
    AuthFailed,
    #[error("Rate limited (HTTP 429): try again later")]
    RateLimited,
    #[error("Server error (HTTP {0}): {1}")]
    ServerError(u16, String),
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
    #[error("Server error (HTTP {0}): {1}")]
    ServerError(u16, String),
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
            _ => UserAction::RetryOrQueue,
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
            _ => UserAction::RetryOrQueue,
        }
    }
}
