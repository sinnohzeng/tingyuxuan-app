use serde::{Deserialize, Serialize};

/// Options for speech-to-text transcription.
///
/// `language` and `prompt` are not used by the current DashScope streaming
/// provider but are reserved for future providers (e.g. Whisper, Azure)
/// that accept these hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTOptions {
    /// Language code: "auto", "en", "zh", etc.
    pub language: Option<String>,
    /// Vocabulary hints to improve recognition accuracy.
    pub prompt: Option<String>,
}
