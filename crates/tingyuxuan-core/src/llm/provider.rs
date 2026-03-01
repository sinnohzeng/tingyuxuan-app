use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::audio::encoder::EncodedAudio;
use crate::context::InputContext;
use crate::error::LLMError;

/// The processing mode determines which LLM prompt template is used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingMode {
    /// Clean up voice transcript into polished written text.
    Dictate,
    /// Translate the transcript into the target language.
    Translate,
    /// Use the transcript as a free-form AI assistant query.
    AiAssistant,
    /// Edit/refine already-selected text based on the voice instruction.
    Edit,
}

impl fmt::Display for ProcessingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessingMode::Dictate => write!(f, "dictate"),
            ProcessingMode::Translate => write!(f, "translate"),
            ProcessingMode::AiAssistant => write!(f, "ai_assistant"),
            ProcessingMode::Edit => write!(f, "edit"),
        }
    }
}

impl FromStr for ProcessingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dictate" => Ok(ProcessingMode::Dictate),
            "translate" => Ok(ProcessingMode::Translate),
            "ai_assistant" => Ok(ProcessingMode::AiAssistant),
            "edit" => Ok(ProcessingMode::Edit),
            other => Err(format!("Unknown processing mode: '{other}'")),
        }
    }
}

/// 多模态处理输入 — 包含编码后的音频和上下文。
#[derive(Debug)]
pub struct ProcessingInput {
    /// Which processing pipeline to use.
    pub mode: ProcessingMode,
    /// 编码后的音频数据（WAV base64）。
    pub audio: EncodedAudio,
    /// 统一上下文模型。
    pub context: InputContext,
    /// Target language code for Translate mode (e.g. "en", "ja").
    pub target_language: Option<String>,
    /// User-defined dictionary terms to improve recognition.
    pub user_dictionary: Vec<String>,
}

/// Result returned from LLM processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResult {
    /// The processed / cleaned-up text.
    pub processed_text: String,
    /// Total tokens consumed (if the provider reports it).
    pub tokens_used: Option<u32>,
}

/// Trait that all LLM providers must implement.
///
/// Rust 2024 edition 原生支持 async fn in trait，无需手动 Pin<Box<dyn Future>>。
pub trait LLMProvider: Send + Sync {
    /// Human-readable name of this provider (e.g. "Multimodal", "DashScope").
    fn name(&self) -> &str;

    /// Process the input through the LLM and return the result.
    fn process(
        &self,
        input: &ProcessingInput,
    ) -> impl std::future::Future<Output = Result<LLMResult, LLMError>> + Send;

    /// Verify that the provider's credentials and endpoint are reachable.
    fn test_connection(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, LLMError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_mode_from_str_all_variants() {
        assert_eq!(
            "dictate".parse::<ProcessingMode>().unwrap(),
            ProcessingMode::Dictate
        );
        assert_eq!(
            "translate".parse::<ProcessingMode>().unwrap(),
            ProcessingMode::Translate
        );
        assert_eq!(
            "ai_assistant".parse::<ProcessingMode>().unwrap(),
            ProcessingMode::AiAssistant
        );
        assert_eq!(
            "edit".parse::<ProcessingMode>().unwrap(),
            ProcessingMode::Edit
        );
    }

    #[test]
    fn test_processing_mode_from_str_invalid() {
        assert!("unknown".parse::<ProcessingMode>().is_err());
        assert!("Dictate".parse::<ProcessingMode>().is_err()); // 大小写敏感
        assert!("".parse::<ProcessingMode>().is_err());
    }

    #[test]
    fn test_processing_mode_display_roundtrip() {
        let modes = [
            ProcessingMode::Dictate,
            ProcessingMode::Translate,
            ProcessingMode::AiAssistant,
            ProcessingMode::Edit,
        ];
        for mode in &modes {
            let s = mode.to_string();
            let parsed: ProcessingMode = s.parse().unwrap();
            assert_eq!(&parsed, mode);
        }
    }

    #[test]
    fn test_processing_mode_display_values() {
        assert_eq!(ProcessingMode::Dictate.to_string(), "dictate");
        assert_eq!(ProcessingMode::Translate.to_string(), "translate");
        assert_eq!(ProcessingMode::AiAssistant.to_string(), "ai_assistant");
        assert_eq!(ProcessingMode::Edit.to_string(), "edit");
    }
}
