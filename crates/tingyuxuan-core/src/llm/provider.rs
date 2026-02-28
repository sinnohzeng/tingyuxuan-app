use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

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

/// Input to the LLM processing step.
#[derive(Debug, Clone)]
pub struct LLMInput {
    /// Which processing pipeline to use.
    pub mode: ProcessingMode,
    /// The raw transcript from STT.
    pub raw_transcript: String,
    /// Target language code for Translate mode (e.g. "en", "ja").
    pub target_language: Option<String>,
    /// 统一上下文模型
    pub context: InputContext,
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
pub trait LLMProvider: Send + Sync {
    /// Human-readable name of this provider (e.g. "OpenAI", "DashScope").
    fn name(&self) -> &str;

    /// Process the input through the LLM and return the result.
    fn process<'a>(
        &'a self,
        input: &'a LLMInput,
    ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>>;

    /// Verify that the provider's credentials and endpoint are reachable.
    fn test_connection(&self) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_mode_from_str_all_variants() {
        assert_eq!("dictate".parse::<ProcessingMode>().unwrap(), ProcessingMode::Dictate);
        assert_eq!("translate".parse::<ProcessingMode>().unwrap(), ProcessingMode::Translate);
        assert_eq!("ai_assistant".parse::<ProcessingMode>().unwrap(), ProcessingMode::AiAssistant);
        assert_eq!("edit".parse::<ProcessingMode>().unwrap(), ProcessingMode::Edit);
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
