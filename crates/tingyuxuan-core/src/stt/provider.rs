use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::STTError;

/// Options for speech-to-text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTOptions {
    /// Language code: "auto", "en", "zh", etc.
    pub language: Option<String>,
    /// Vocabulary hints to improve recognition accuracy.
    pub prompt: Option<String>,
}

/// Result of a speech-to-text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTResult {
    /// The transcribed text.
    pub text: String,
    /// Detected or specified language code.
    pub language: String,
    /// Duration of the audio in seconds.
    pub duration_seconds: f64,
}

/// Trait for speech-to-text providers.
#[async_trait]
pub trait STTProvider: Send + Sync {
    /// Returns the name of this provider.
    fn name(&self) -> &str;

    /// Transcribe the audio file at the given path.
    async fn transcribe(
        &self,
        audio_path: &Path,
        options: &STTOptions,
    ) -> Result<STTResult, STTError>;

    /// Test that the provider connection and credentials are valid.
    async fn test_connection(&self) -> Result<bool, STTError>;
}
