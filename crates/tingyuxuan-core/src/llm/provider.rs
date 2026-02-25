use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

use crate::error::LLMError;

/// The processing mode determines which LLM prompt template is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Input to the LLM processing step.
#[derive(Debug, Clone)]
pub struct LLMInput {
    /// Which processing pipeline to use.
    pub mode: ProcessingMode,
    /// The raw transcript from STT.
    pub raw_transcript: String,
    /// Target language code for Translate mode (e.g. "en", "ja").
    pub target_language: Option<String>,
    /// Text currently selected in the user's application (for Edit mode).
    pub selected_text: Option<String>,
    /// Name of the currently focused application (for context hints).
    pub current_app: Option<String>,
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
