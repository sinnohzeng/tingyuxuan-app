use std::path::Path;

use tokio::sync::broadcast;

use crate::error::PipelineError;
use crate::llm::provider::{LLMInput, LLMProvider, ProcessingMode};
use crate::pipeline::events::PipelineEvent;
use crate::pipeline::retry::{execute_with_retry, RetryPolicy};
use crate::stt::provider::{STTOptions, STTProvider};

/// Main pipeline orchestrator that coordinates STT and LLM processing.
///
/// Holds trait-object references to the active STT and LLM providers, a
/// broadcast channel for emitting progress events, and a retry policy.
pub struct Pipeline {
    stt: Box<dyn STTProvider>,
    llm: Box<dyn LLMProvider>,
    event_tx: broadcast::Sender<PipelineEvent>,
    retry_policy: RetryPolicy,
}

impl Pipeline {
    /// Create a new pipeline.
    ///
    /// * `stt`      - The speech-to-text provider to use.
    /// * `llm`      - The LLM provider to use.
    /// * `event_tx` - Broadcast sender for pipeline events.
    pub fn new(
        stt: Box<dyn STTProvider>,
        llm: Box<dyn LLMProvider>,
        event_tx: broadcast::Sender<PipelineEvent>,
    ) -> Self {
        Self {
            stt,
            llm,
            event_tx,
            retry_policy: RetryPolicy::default(),
        }
    }

    /// Subscribe to pipeline events.
    pub fn subscribe(&self) -> broadcast::Receiver<PipelineEvent> {
        self.event_tx.subscribe()
    }

    /// Emit an event (best-effort; if there are no receivers we silently drop).
    fn emit(&self, event: PipelineEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Run the full audio processing pipeline:
    ///
    /// 1. Transcribe the audio file via STT (with retry).
    /// 2. Process the raw transcript via LLM (with retry).
    /// 3. Return the final processed text.
    ///
    /// Pipeline events are emitted at each stage so the UI can update in
    /// real time.
    pub async fn process_audio(
        &self,
        audio_path: &Path,
        mode: ProcessingMode,
        app_context: Option<String>,
    ) -> Result<String, PipelineError> {
        // ------------------------------------------------------------------
        // Stage 1: Speech-to-Text
        // ------------------------------------------------------------------
        self.emit(PipelineEvent::TranscriptionStarted);

        let stt_options = STTOptions {
            language: None,
            prompt: None,
        };

        let stt_result = {
            let stt = &self.stt;
            let opts = &stt_options;
            execute_with_retry(&self.retry_policy, || async {
                stt.transcribe(audio_path, opts).await
            })
            .await
        };

        let stt_result = match stt_result {
            Ok(r) => r,
            Err(stt_err) => {
                self.emit(PipelineEvent::Error {
                    message: stt_err.to_string(),
                    user_action: stt_err.user_action(),
                    raw_text: None,
                });
                return Err(PipelineError::Stt(stt_err));
            }
        };

        let raw_text = stt_result.text.clone();
        self.emit(PipelineEvent::TranscriptionComplete {
            raw_text: raw_text.clone(),
        });

        // ------------------------------------------------------------------
        // Stage 2: LLM Processing
        // ------------------------------------------------------------------
        self.emit(PipelineEvent::ProcessingStarted);

        let llm_input = LLMInput {
            mode,
            raw_transcript: raw_text.clone(),
            target_language: None,
            selected_text: None,
            current_app: app_context,
            user_dictionary: Vec::new(),
        };

        let llm_result = {
            let llm = &self.llm;
            let input = &llm_input;
            execute_with_retry(&self.retry_policy, || async {
                llm.process(input).await
            })
            .await
        };

        let llm_result = match llm_result {
            Ok(r) => r,
            Err(llm_err) => {
                self.emit(PipelineEvent::Error {
                    message: llm_err.to_string(),
                    user_action: llm_err.user_action(),
                    raw_text: Some(raw_text),
                });
                return Err(PipelineError::Llm(llm_err));
            }
        };

        let processed = llm_result.processed_text.clone();
        self.emit(PipelineEvent::ProcessingComplete {
            processed_text: processed.clone(),
        });

        Ok(processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::error::{LLMError, STTError};
    use crate::llm::provider::LLMResult;
    use crate::stt::provider::STTResult;

    // -- Mock STT provider --------------------------------------------------

    struct MockSTT {
        text: String,
    }

    #[async_trait]
    impl STTProvider for MockSTT {
        fn name(&self) -> &str {
            "mock-stt"
        }
        async fn transcribe(
            &self,
            _audio_path: &Path,
            _options: &STTOptions,
        ) -> Result<STTResult, STTError> {
            Ok(STTResult {
                text: self.text.clone(),
                language: "zh".to_string(),
                duration_seconds: 3.0,
            })
        }
        async fn test_connection(&self) -> Result<bool, STTError> {
            Ok(true)
        }
    }

    struct FailingSTT;

    #[async_trait]
    impl STTProvider for FailingSTT {
        fn name(&self) -> &str {
            "failing-stt"
        }
        async fn transcribe(
            &self,
            _audio_path: &Path,
            _options: &STTOptions,
        ) -> Result<STTResult, STTError> {
            Err(STTError::Timeout(15))
        }
        async fn test_connection(&self) -> Result<bool, STTError> {
            Ok(false)
        }
    }

    // -- Mock LLM provider --------------------------------------------------

    struct MockLLM {
        response: String,
    }

    #[async_trait]
    impl LLMProvider for MockLLM {
        fn name(&self) -> &str {
            "mock-llm"
        }
        async fn process(&self, _input: &LLMInput) -> Result<LLMResult, LLMError> {
            Ok(LLMResult {
                processed_text: self.response.clone(),
                tokens_used: Some(42),
            })
        }
        async fn test_connection(&self) -> Result<bool, LLMError> {
            Ok(true)
        }
    }

    struct FailingLLM;

    #[async_trait]
    impl LLMProvider for FailingLLM {
        fn name(&self) -> &str {
            "failing-llm"
        }
        async fn process(&self, _input: &LLMInput) -> Result<LLMResult, LLMError> {
            Err(LLMError::Timeout)
        }
        async fn test_connection(&self) -> Result<bool, LLMError> {
            Ok(false)
        }
    }

    // -- Helper -------------------------------------------------------------

    fn make_pipeline(
        stt: Box<dyn STTProvider>,
        llm: Box<dyn LLMProvider>,
    ) -> (Pipeline, broadcast::Receiver<PipelineEvent>) {
        let (tx, rx) = broadcast::channel(32);
        let pipeline = Pipeline::new(stt, llm, tx);
        (pipeline, rx)
    }

    // -- Tests --------------------------------------------------------------

    #[tokio::test]
    async fn test_happy_path() {
        let (pipeline, _rx) = make_pipeline(
            Box::new(MockSTT {
                text: "嗯 你好世界".to_string(),
            }),
            Box::new(MockLLM {
                response: "你好，世界。".to_string(),
            }),
        );

        let result = pipeline
            .process_audio(
                Path::new("/tmp/fake.wav"),
                ProcessingMode::Dictate,
                None,
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "你好，世界。");
    }

    #[tokio::test]
    async fn test_stt_failure_emits_error() {
        let (pipeline, mut rx) = make_pipeline(
            Box::new(FailingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let result = pipeline
            .process_audio(
                Path::new("/tmp/fake.wav"),
                ProcessingMode::Dictate,
                None,
            )
            .await;

        assert!(result.is_err());

        // Drain events and find the Error event.
        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if let PipelineEvent::Error {
                raw_text,
                ..
            } = event
            {
                // STT failure should not include raw_text.
                assert!(raw_text.is_none());
                found_error = true;
            }
        }
        assert!(found_error);
    }

    #[tokio::test]
    async fn test_llm_failure_includes_raw_text() {
        let (pipeline, mut rx) = make_pipeline(
            Box::new(MockSTT {
                text: "raw transcript".to_string(),
            }),
            Box::new(FailingLLM),
        );

        let result = pipeline
            .process_audio(
                Path::new("/tmp/fake.wav"),
                ProcessingMode::Dictate,
                None,
            )
            .await;

        assert!(result.is_err());

        // Drain events and find the Error event.
        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if let PipelineEvent::Error {
                raw_text,
                ..
            } = event
            {
                // LLM failure should include the raw transcript.
                assert_eq!(raw_text, Some("raw transcript".to_string()));
                found_error = true;
            }
        }
        assert!(found_error);
    }

    #[test]
    fn test_subscribe() {
        let (tx, _) = broadcast::channel(16);
        let pipeline = Pipeline::new(
            Box::new(MockSTT {
                text: String::new(),
            }),
            Box::new(MockLLM {
                response: String::new(),
            }),
            tx,
        );

        let _rx = pipeline.subscribe();
        // Just verify it compiles and doesn't panic.
    }
}
