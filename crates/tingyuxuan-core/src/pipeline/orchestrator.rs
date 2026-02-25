use std::path::PathBuf;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::error::PipelineError;
use crate::llm::provider::{LLMInput, LLMProvider, ProcessingMode};
use crate::pipeline::events::PipelineEvent;
use crate::pipeline::retry::{RetryPolicy, execute_with_retry};
use crate::stt::provider::{STTOptions, STTProvider};

/// Extensible request struct for pipeline processing.
///
/// New fields can be added without breaking existing callers by providing
/// sensible defaults.
#[derive(Debug, Clone)]
pub struct ProcessingRequest {
    pub audio_path: PathBuf,
    pub mode: ProcessingMode,
    pub app_context: Option<String>,
    pub target_language: Option<String>,
    pub selected_text: Option<String>,
    pub user_dictionary: Vec<String>,
}

impl ProcessingRequest {
    /// Create a minimal request for dictation mode.
    pub fn dictate(audio_path: impl Into<PathBuf>) -> Self {
        Self {
            audio_path: audio_path.into(),
            mode: ProcessingMode::Dictate,
            app_context: None,
            target_language: None,
            selected_text: None,
            user_dictionary: Vec::new(),
        }
    }
}

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
    /// real time.  Pass a `CancellationToken` to allow the caller to abort
    /// processing at any stage.
    pub async fn process_audio(
        &self,
        request: &ProcessingRequest,
        cancel_token: CancellationToken,
    ) -> Result<String, PipelineError> {
        // ------------------------------------------------------------------
        // Stage 1: Speech-to-Text
        // ------------------------------------------------------------------
        if cancel_token.is_cancelled() {
            return Err(PipelineError::Cancelled);
        }

        self.emit(PipelineEvent::TranscriptionStarted);

        let stt_options = STTOptions {
            language: None,
            prompt: None,
        };

        let audio_path = &request.audio_path;
        let stt_result = {
            let stt = &self.stt;
            let opts = &stt_options;
            execute_with_retry(&self.retry_policy, &cancel_token, || async {
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
        if cancel_token.is_cancelled() {
            return Err(PipelineError::Cancelled);
        }

        self.emit(PipelineEvent::ProcessingStarted);

        let llm_input = LLMInput {
            mode: request.mode.clone(),
            raw_transcript: raw_text.clone(),
            target_language: request.target_language.clone(),
            selected_text: request.selected_text.clone(),
            current_app: request.app_context.clone(),
            user_dictionary: request.user_dictionary.clone(),
        };

        let llm_result = {
            let llm = &self.llm;
            let input = &llm_input;
            execute_with_retry(&self.retry_policy, &cancel_token, || async {
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
    use crate::error::{LLMError, STTError};
    use crate::llm::provider::LLMResult;
    use crate::stt::provider::STTResult;
    use std::future::Future;
    use std::path::Path;
    use std::pin::Pin;

    // -- Mock STT provider --------------------------------------------------

    struct MockSTT {
        text: String,
    }

    impl STTProvider for MockSTT {
        fn name(&self) -> &str {
            "mock-stt"
        }
        fn transcribe<'a>(
            &'a self,
            _audio_path: &'a Path,
            _options: &'a STTOptions,
        ) -> Pin<Box<dyn Future<Output = Result<STTResult, STTError>> + Send + 'a>> {
            Box::pin(async move {
                Ok(STTResult {
                    text: self.text.clone(),
                    language: "zh".to_string(),
                    duration_seconds: 3.0,
                })
            })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
        }
    }

    struct FailingSTT;

    impl STTProvider for FailingSTT {
        fn name(&self) -> &str {
            "failing-stt"
        }
        fn transcribe<'a>(
            &'a self,
            _audio_path: &'a Path,
            _options: &'a STTOptions,
        ) -> Pin<Box<dyn Future<Output = Result<STTResult, STTError>> + Send + 'a>> {
            Box::pin(async { Err(STTError::Timeout(15)) })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
            Box::pin(async { Ok(false) })
        }
    }

    // -- Mock LLM provider --------------------------------------------------

    struct MockLLM {
        response: String,
    }

    impl LLMProvider for MockLLM {
        fn name(&self) -> &str {
            "mock-llm"
        }
        fn process<'a>(
            &'a self,
            _input: &'a LLMInput,
        ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>> {
            Box::pin(async move {
                Ok(LLMResult {
                    processed_text: self.response.clone(),
                    tokens_used: Some(42),
                })
            })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
        }
    }

    struct FailingLLM;

    impl LLMProvider for FailingLLM {
        fn name(&self) -> &str {
            "failing-llm"
        }
        fn process<'a>(
            &'a self,
            _input: &'a LLMInput,
        ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>> {
            Box::pin(async { Err(LLMError::Timeout) })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>> {
            Box::pin(async { Ok(false) })
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

        let request = ProcessingRequest::dictate("/tmp/fake.wav");
        let token = CancellationToken::new();
        let result = pipeline.process_audio(&request, token).await;

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

        let request = ProcessingRequest::dictate("/tmp/fake.wav");
        let token = CancellationToken::new();
        let result = pipeline.process_audio(&request, token).await;

        assert!(result.is_err());

        // Drain events and find the Error event.
        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if let PipelineEvent::Error { raw_text, .. } = event {
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

        let request = ProcessingRequest::dictate("/tmp/fake.wav");
        let token = CancellationToken::new();
        let result = pipeline.process_audio(&request, token).await;

        assert!(result.is_err());

        // Drain events and find the Error event.
        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if let PipelineEvent::Error { raw_text, .. } = event {
                // LLM failure should include the raw transcript.
                assert_eq!(raw_text, Some("raw transcript".to_string()));
                found_error = true;
            }
        }
        assert!(found_error);
    }

    #[tokio::test]
    async fn test_cancellation_before_stt() {
        let (pipeline, _rx) = make_pipeline(
            Box::new(MockSTT {
                text: "should not reach".to_string(),
            }),
            Box::new(MockLLM {
                response: "should not reach".to_string(),
            }),
        );

        let request = ProcessingRequest::dictate("/tmp/fake.wav");
        let token = CancellationToken::new();
        token.cancel();

        let result = pipeline.process_audio(&request, token).await;
        assert!(matches!(result, Err(PipelineError::Cancelled)));
    }

    #[tokio::test]
    async fn test_processing_request_with_translate() {
        let (pipeline, _rx) = make_pipeline(
            Box::new(MockSTT {
                text: "你好世界".to_string(),
            }),
            Box::new(MockLLM {
                response: "Hello, world.".to_string(),
            }),
        );

        let request = ProcessingRequest {
            audio_path: PathBuf::from("/tmp/fake.wav"),
            mode: ProcessingMode::Translate,
            app_context: None,
            target_language: Some("en".to_string()),
            selected_text: None,
            user_dictionary: Vec::new(),
        };
        let token = CancellationToken::new();
        let result = pipeline.process_audio(&request, token).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world.");
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
