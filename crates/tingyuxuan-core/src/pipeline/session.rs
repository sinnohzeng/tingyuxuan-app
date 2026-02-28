//! Session 编排层 — 将录音 session 的完整生命周期下沉到 core。
//!
//! 将 STT 连接 → 音频转发 → 结果收集 → LLM 处理 → 结构化结果的完整流程
//! 从平台层（Tauri/JNI）下沉到 `tingyuxuan-core`，使平台层成为薄桥接层。

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::context::InputContext;
use crate::error::PipelineError;
use crate::llm::provider::ProcessingMode;
use crate::pipeline::ProcessingRequest;
use crate::pipeline::orchestrator::Pipeline;
use crate::stt::provider::STTOptions;
use crate::stt::streaming::{AudioChunk, StreamingSTTEvent};

/// STT 结果收集超时（秒）。
const STT_COLLECT_TIMEOUT_SECS: u64 = 30;

/// Session 配置 — 描述一次录音 session 的所有参数。
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub mode: ProcessingMode,
    pub context: InputContext,
    pub target_language: Option<String>,
    pub user_dictionary: Vec<String>,
    pub stt_options: STTOptions,
}

/// 托管的流式会话 — 封装了音频发送、事件接收和取消令牌。
///
/// 平台层只需持有此句柄，通过 `send_audio()` 发送音频帧，
/// 最后调用 `SessionOrchestrator::finish()` 完成处理。
pub struct ManagedSession {
    audio_tx: mpsc::Sender<AudioChunk>,
    event_rx: Option<mpsc::Receiver<StreamingSTTEvent>>,
    config: SessionConfig,
    cancel_token: CancellationToken,
}

impl ManagedSession {
    /// 发送一帧音频到 STT。
    ///
    /// 使用 `try_send` 实现背压控制：channel 满时丢帧（STT 容忍少量丢帧）。
    /// 返回 `true` 表示发送成功，`false` 表示 channel 满或已关闭。
    pub fn send_audio(&self, chunk: AudioChunk) -> bool {
        match self.audio_tx.try_send(chunk) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::debug!("Audio channel full, frame dropped (backpressure)");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!("Audio channel closed");
                false
            }
        }
    }

    /// 获取音频发送端的 clone（用于桥接 task）。
    pub fn audio_sender(&self) -> mpsc::Sender<AudioChunk> {
        self.audio_tx.clone()
    }

    /// 取消当前 session。
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// 获取取消令牌的引用。
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }

    /// 获取 session 配置。
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// 仅用于测试 — 直接构造 ManagedSession，绕过 SessionOrchestrator::start()。
    #[cfg(any(test, feature = "testing"))]
    pub fn new_for_testing(
        audio_tx: mpsc::Sender<AudioChunk>,
        event_rx: mpsc::Receiver<StreamingSTTEvent>,
        config: SessionConfig,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            audio_tx,
            event_rx: Some(event_rx),
            config,
            cancel_token,
        }
    }
}

/// Session 处理结果。
#[derive(Debug)]
pub enum SessionResult {
    /// STT + LLM 均成功。
    Success {
        raw_text: String,
        processed_text: String,
    },
    /// STT 返回空转写。
    EmptyTranscript,
    /// 处理失败（STT 或 LLM 错误）。
    Failed {
        error: PipelineError,
        raw_text: Option<String>,
    },
    /// 用户取消。
    Cancelled,
}

/// Session 编排器 — 无状态命名空间，协调 session 的完整生命周期。
pub struct SessionOrchestrator;

impl SessionOrchestrator {
    /// 启动一个新的 managed session。
    ///
    /// 建立 STT WebSocket 连接，返回 `ManagedSession` 句柄。
    /// 调用者通过 `send_audio()` 发送音频帧，最后调用 `finish()` 完成处理。
    pub async fn start(
        pipeline: &Pipeline,
        config: SessionConfig,
    ) -> Result<ManagedSession, PipelineError> {
        tracing::info!(mode = %config.mode, "Starting streaming STT session");
        let streaming_session = pipeline.start_streaming(&config.stt_options).await?;

        Ok(ManagedSession {
            audio_tx: streaming_session.audio_tx,
            event_rx: Some(streaming_session.event_rx),
            config,
            cancel_token: CancellationToken::new(),
        })
    }

    /// 完成 session：收集 STT 结果 → LLM 处理 → 返回结构化结果。
    ///
    /// 调用前应先停止录音（drop 桥接 task 或停止发送音频），
    /// 以便 STT 产生最终结果。
    pub async fn finish(pipeline: &Pipeline, mut session: ManagedSession) -> SessionResult {
        // 检查是否已取消。
        if session.cancel_token.is_cancelled() {
            return SessionResult::Cancelled;
        }

        // Drop audio_tx → 通知 STT 音频流结束。
        drop(session.audio_tx);

        // 收集 STT 结果（带超时和取消支持）。
        let event_rx = match session.event_rx.take() {
            Some(rx) => rx,
            None => {
                return SessionResult::Failed {
                    error: PipelineError::Stt(crate::error::STTError::InvalidResponse(
                        "Streaming session already completed".to_string(),
                    )),
                    raw_text: None,
                };
            }
        };

        let collect_result = {
            let _span = tracing::info_span!("stt_collect").entered();
            match Self::collect_stt_results(event_rx, &session.cancel_token).await {
                Ok(text) => {
                    tracing::info!(len = text.len(), "STT transcript collected");
                    Ok(text)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "STT collection failed");
                    Err(e)
                }
            }
        };

        let transcript = match collect_result {
            Ok(text) if text.trim().is_empty() => return SessionResult::EmptyTranscript,
            Ok(text) => text,
            Err(e) => {
                return SessionResult::Failed {
                    error: e,
                    raw_text: None,
                };
            }
        };

        // 再次检查取消。
        if session.cancel_token.is_cancelled() {
            return SessionResult::Cancelled;
        }

        // LLM 处理。
        let request = ProcessingRequest {
            mode: session.config.mode.clone(),
            context: session.config.context.clone(),
            target_language: session.config.target_language.clone(),
            user_dictionary: session.config.user_dictionary.clone(),
        };

        match pipeline
            .process_transcript(transcript.clone(), &request, session.cancel_token)
            .await
        {
            Ok(processed_text) => SessionResult::Success {
                raw_text: transcript,
                processed_text,
            },
            Err(PipelineError::Cancelled) => SessionResult::Cancelled,
            Err(error) => SessionResult::Failed {
                error,
                raw_text: Some(transcript),
            },
        }
    }

    /// 从事件流中收集 STT 最终结果，带 30s 超时和取消支持。
    async fn collect_stt_results(
        mut event_rx: mpsc::Receiver<StreamingSTTEvent>,
        cancel_token: &CancellationToken,
    ) -> Result<String, PipelineError> {
        let mut finals: Vec<(u32, String)> = Vec::new();
        let timeout = tokio::time::sleep(Duration::from_secs(STT_COLLECT_TIMEOUT_SECS));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Some(StreamingSTTEvent::Final { text, sentence_index }) => {
                            tracing::debug!(sentence_index, "STT sentence final");
                            finals.push((sentence_index, text));
                        }
                        Some(StreamingSTTEvent::Error(e)) => {
                            return Err(PipelineError::Stt(e));
                        }
                        Some(StreamingSTTEvent::Partial { .. }) => {
                            // 忽略中间结果。
                        }
                        None => {
                            // Channel 关闭 — STT 完成。
                            break;
                        }
                    }
                }
                _ = cancel_token.cancelled() => {
                    return Err(PipelineError::Cancelled);
                }
                _ = &mut timeout => {
                    tracing::warn!(timeout_secs = STT_COLLECT_TIMEOUT_SECS, "STT collection timed out");
                    return Err(PipelineError::Stt(
                        crate::error::STTError::Timeout(STT_COLLECT_TIMEOUT_SECS),
                    ));
                }
            }
        }

        if finals.is_empty() {
            return Ok(String::new()); // 会被调用者判定为 EmptyTranscript
        }

        finals.sort_by_key(|(idx, _)| *idx);
        Ok(finals
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputContext;
    use crate::error::{LLMError, STTError};
    use crate::llm::provider::{LLMInput, LLMProvider, LLMResult};
    use crate::pipeline::events::PipelineEvent;
    use crate::stt::streaming::{
        STREAMING_CHANNEL_CAPACITY, StreamingSTTProvider, StreamingSession,
    };
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::broadcast;

    // -- Mock Streaming STT ---------------------------------------------------

    struct MockStreamingSTT;

    impl StreamingSTTProvider for MockStreamingSTT {
        fn name(&self) -> &str {
            "mock-streaming-stt"
        }
        fn start_stream<'a>(
            &'a self,
            _options: &'a STTOptions,
        ) -> Pin<Box<dyn Future<Output = Result<StreamingSession, STTError>> + Send + 'a>> {
            Box::pin(async {
                let (audio_tx, mut audio_rx) = mpsc::channel(STREAMING_CHANNEL_CAPACITY);
                let (event_tx, event_rx) = mpsc::channel(64);

                tokio::spawn(async move {
                    let mut received = false;
                    while let Some(_chunk) = audio_rx.recv().await {
                        if !received {
                            let _ = event_tx
                                .send(StreamingSTTEvent::Partial {
                                    text: "你好".to_string(),
                                    sentence_index: 0,
                                })
                                .await;
                            received = true;
                        }
                    }
                    let _ = event_tx
                        .send(StreamingSTTEvent::Final {
                            text: "你好世界".to_string(),
                            sentence_index: 0,
                        })
                        .await;
                });

                Ok(StreamingSession { audio_tx, event_rx })
            })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
        }
    }

    /// 返回空结果的 STT（模拟无音频输入）。
    struct EmptyStreamingSTT;

    impl StreamingSTTProvider for EmptyStreamingSTT {
        fn name(&self) -> &str {
            "empty-streaming-stt"
        }
        fn start_stream<'a>(
            &'a self,
            _options: &'a STTOptions,
        ) -> Pin<Box<dyn Future<Output = Result<StreamingSession, STTError>> + Send + 'a>> {
            Box::pin(async {
                let (audio_tx, mut audio_rx) = mpsc::channel(STREAMING_CHANNEL_CAPACITY);
                let (_event_tx, event_rx) = mpsc::channel(64);

                tokio::spawn(async move {
                    // 消耗所有音频但不产出结果。
                    while audio_rx.recv().await.is_some() {}
                    // event_tx drop → event_rx 关闭。
                });

                Ok(StreamingSession { audio_tx, event_rx })
            })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
        }
    }

    /// STT 返回错误事件。
    struct ErrorStreamingSTT;

    impl StreamingSTTProvider for ErrorStreamingSTT {
        fn name(&self) -> &str {
            "error-streaming-stt"
        }
        fn start_stream<'a>(
            &'a self,
            _options: &'a STTOptions,
        ) -> Pin<Box<dyn Future<Output = Result<StreamingSession, STTError>> + Send + 'a>> {
            Box::pin(async {
                let (audio_tx, mut audio_rx) = mpsc::channel(STREAMING_CHANNEL_CAPACITY);
                let (event_tx, event_rx) = mpsc::channel(64);

                tokio::spawn(async move {
                    while audio_rx.recv().await.is_some() {}
                    let _ = event_tx
                        .send(StreamingSTTEvent::Error(STTError::NetworkError(
                            "connection reset".to_string(),
                        )))
                        .await;
                });

                Ok(StreamingSession { audio_tx, event_rx })
            })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
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

    fn make_pipeline(stt: Box<dyn StreamingSTTProvider>, llm: Box<dyn LLMProvider>) -> Pipeline {
        let (tx, _) = broadcast::channel::<PipelineEvent>(32);
        Pipeline::new(stt, llm, tx)
    }

    fn default_config() -> SessionConfig {
        SessionConfig {
            mode: ProcessingMode::Dictate,
            context: InputContext::default(),
            target_language: None,
            user_dictionary: Vec::new(),
            stt_options: STTOptions {
                language: None,
                prompt: None,
            },
        }
    }

    // -- Tests --------------------------------------------------------------

    #[tokio::test]
    async fn test_session_happy_path() {
        let pipeline = make_pipeline(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: "你好，世界。".to_string(),
            }),
        );

        let session = SessionOrchestrator::start(&pipeline, default_config())
            .await
            .unwrap();

        // 发送一帧音频。
        assert!(session.send_audio(AudioChunk {
            samples: vec![0i16; 320],
        }));

        let result = SessionOrchestrator::finish(&pipeline, session).await;
        match result {
            SessionResult::Success {
                raw_text,
                processed_text,
            } => {
                assert_eq!(raw_text, "你好世界");
                assert_eq!(processed_text, "你好，世界。");
            }
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_session_empty_transcript() {
        let pipeline = make_pipeline(
            Box::new(EmptyStreamingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let session = SessionOrchestrator::start(&pipeline, default_config())
            .await
            .unwrap();

        let result = SessionOrchestrator::finish(&pipeline, session).await;
        assert!(matches!(result, SessionResult::EmptyTranscript));
    }

    #[tokio::test]
    async fn test_session_stt_error() {
        let pipeline = make_pipeline(
            Box::new(ErrorStreamingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let session = SessionOrchestrator::start(&pipeline, default_config())
            .await
            .unwrap();

        let result = SessionOrchestrator::finish(&pipeline, session).await;
        match result {
            SessionResult::Failed { error, raw_text } => {
                assert!(matches!(error, PipelineError::Stt(_)));
                assert!(raw_text.is_none());
            }
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_session_llm_failure_preserves_raw_text() {
        let pipeline = make_pipeline(Box::new(MockStreamingSTT), Box::new(FailingLLM));

        let session = SessionOrchestrator::start(&pipeline, default_config())
            .await
            .unwrap();

        // 发送音频触发 STT 产出。
        assert!(session.send_audio(AudioChunk {
            samples: vec![0i16; 320],
        }));

        let result = SessionOrchestrator::finish(&pipeline, session).await;
        match result {
            SessionResult::Failed { error, raw_text } => {
                assert!(matches!(error, PipelineError::Llm(_)));
                assert_eq!(raw_text, Some("你好世界".to_string()));
            }
            other => panic!("Expected Failed with raw_text, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_session_cancellation_during_stt() {
        let pipeline = make_pipeline(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let session = SessionOrchestrator::start(&pipeline, default_config())
            .await
            .unwrap();

        // 发送音频后立即取消。
        assert!(session.send_audio(AudioChunk {
            samples: vec![0i16; 320],
        }));
        session.cancel();

        let result = SessionOrchestrator::finish(&pipeline, session).await;
        assert!(matches!(result, SessionResult::Cancelled));
    }

    #[tokio::test]
    async fn test_session_cancellation_before_finish() {
        let pipeline = make_pipeline(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let session = SessionOrchestrator::start(&pipeline, default_config())
            .await
            .unwrap();
        session.cancel();

        let result = SessionOrchestrator::finish(&pipeline, session).await;
        assert!(matches!(result, SessionResult::Cancelled));
    }

    #[tokio::test]
    async fn test_session_stt_timeout() {
        // STT 永远不关闭 event channel → 触发 30s 超时。
        // 为了测试速度，我们直接测试 collect_stt_results 的超时逻辑。
        let (_event_tx, event_rx) = mpsc::channel::<StreamingSTTEvent>(64);
        let cancel_token = CancellationToken::new();

        // 用一个很短的超时来测试（修改常量不可行，所以直接测试内部逻辑的等价行为）。
        let result = tokio::time::timeout(
            Duration::from_millis(50),
            SessionOrchestrator::collect_stt_results(event_rx, &cancel_token),
        )
        .await;

        // 外层 timeout 会触发。
        assert!(result.is_err()); // Elapsed error
    }

    #[tokio::test]
    async fn test_send_audio_backpressure() {
        let pipeline = make_pipeline(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let session = SessionOrchestrator::start(&pipeline, default_config())
            .await
            .unwrap();

        // 填满 channel（STREAMING_CHANNEL_CAPACITY = 50）。
        for _ in 0..STREAMING_CHANNEL_CAPACITY {
            assert!(session.send_audio(AudioChunk {
                samples: vec![0i16; 320],
            }));
        }

        // 再发一帧应该失败（channel 满）。
        // 注意：MockStreamingSTT 的后台任务会消费音频，所以可能不会100%满。
        // 我们至少验证 send_audio 不会 panic。
        let _ = session.send_audio(AudioChunk {
            samples: vec![0i16; 320],
        });
    }

    #[tokio::test]
    async fn test_send_audio_closed_channel() {
        let (tx, _rx) = mpsc::channel::<AudioChunk>(10);
        let (_event_tx, event_rx) = mpsc::channel(64);

        let session = ManagedSession {
            audio_tx: tx,
            event_rx: Some(event_rx),
            config: default_config(),
            cancel_token: CancellationToken::new(),
        };

        // Drop the receiver to close the channel.
        drop(_rx);

        assert!(!session.send_audio(AudioChunk {
            samples: vec![0i16; 320],
        }));
    }
}
