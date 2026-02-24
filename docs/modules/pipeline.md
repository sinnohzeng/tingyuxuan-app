# Pipeline 模块（处理流水线）

## 模块职责

Pipeline 模块负责协调 STT -> LLM 的完整处理流水线，提供重试、取消、事件广播、离线队列、网络监控和崩溃恢复等功能。作为核心调度层，衔接音频录制与最终文本输出之间的所有异步处理步骤。

---

## 核心类型定义

### Pipeline（编排器）

```rust
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
```

### ProcessingRequest

```rust
/// Extensible request struct for pipeline processing.
#[derive(Debug, Clone)]
pub struct ProcessingRequest {
    pub audio_path: PathBuf,
    pub mode: ProcessingMode,
    pub app_context: Option<String>,
    pub target_language: Option<String>,
    pub selected_text: Option<String>,
    pub user_dictionary: Vec<String>,
}
```

### PipelineEvent

```rust
/// Events emitted by the pipeline as it progresses through each stage.
///
/// The frontend subscribes to these via a `broadcast::Receiver` and updates
/// the floating-bar UI accordingly.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PipelineEvent {
    /// Recording has started for a new session.
    RecordingStarted { session_id: String, mode: String },
    /// Real-time microphone volume levels (for waveform visualization).
    VolumeUpdate { levels: Vec<f32> },
    /// Recording stopped; includes the total duration.
    RecordingStopped { duration_ms: u64 },
    /// STT transcription has been submitted.
    TranscriptionStarted,
    /// STT transcription completed successfully.
    TranscriptionComplete { raw_text: String },
    /// LLM processing has started.
    ProcessingStarted,
    /// LLM processing completed successfully.
    ProcessingComplete { processed_text: String },
    /// An error occurred at some pipeline stage.
    Error {
        message: String,
        user_action: UserAction,
        /// When LLM fails but STT succeeded, this carries the raw transcript
        /// so the user can choose to insert it directly.
        raw_text: Option<String>,
    },
    /// Network reachability changed.
    NetworkStatusChanged { online: bool },
    /// The recording was saved to the offline queue for later processing.
    QueuedForLater { session_id: String },
    /// The current recording was cancelled by the user.
    RecordingCancelled,
}
```

### OfflineQueue

```rust
/// A recording that was captured while the device was offline and is waiting
/// to be processed once connectivity is restored.
#[derive(Debug, Clone)]
pub struct QueuedRecording {
    pub session_id: String,
    pub audio_path: PathBuf,
    pub mode: ProcessingMode,
    pub target_language: Option<String>,
    pub selected_text: Option<String>,
    pub app_context: Option<String>,
}

/// A simple in-memory FIFO queue of recordings captured while offline.
pub struct OfflineQueue {
    items: Vec<QueuedRecording>,
}
```

### RetryPolicy

```rust
/// Configuration for retry behaviour with exponential back-off.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 = no retries, 1 = one retry, etc.).
    pub max_retries: u32,          // 默认: 1
    /// Initial delay before the first retry, in milliseconds.
    pub initial_delay_ms: u64,     // 默认: 3000
    /// Multiplier applied to the delay after each retry.
    pub backoff_factor: f64,       // 默认: 2.0
}
```

### NetworkMonitor

```rust
/// Periodically checks network connectivity by issuing an HTTP HEAD request
/// and emits `PipelineEvent::NetworkStatusChanged` whenever the reachability
/// state changes.
pub struct NetworkMonitor {
    check_url: String,
    interval: Duration,   // 默认: 30s
    timeout: Duration,    // 默认: 5s
}
```

### RecoveryInfo

```rust
/// Information about a recording session that was interrupted (e.g. by a crash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryInfo {
    /// Path to the orphaned audio file.
    pub audio_path: PathBuf,
    /// ISO-8601 timestamp from the sidecar metadata.
    pub timestamp: String,
    /// Estimated recording duration in milliseconds (from metadata, if available).
    pub duration_estimate: Option<u64>,
    /// The processing mode that was active when the recording started.
    pub mode: String,
}
```

### PipelineError

```rust
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
```

---

## 公开 API

### Pipeline（编排器）

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(stt: Box<dyn STTProvider>, llm: Box<dyn LLMProvider>, event_tx: broadcast::Sender<PipelineEvent>) -> Self` | 创建 Pipeline 实例，使用默认 RetryPolicy |
| `subscribe()` | `fn subscribe(&self) -> broadcast::Receiver<PipelineEvent>` | 订阅 pipeline 事件 |
| `process_audio()` | `async fn process_audio(&self, request: &ProcessingRequest, cancel_token: CancellationToken) -> Result<String, PipelineError>` | 执行完整的 STT -> LLM 流水线 |

### ProcessingRequest

| 方法 | 签名 | 说明 |
|------|------|------|
| `dictate()` | `fn dictate(audio_path: impl Into<PathBuf>) -> Self` | 创建 Dictate 模式的最简请求 |

### OfflineQueue

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new() -> Self` | 创建空队列 |
| `enqueue()` | `fn enqueue(&mut self, recording: QueuedRecording)` | 添加录音到队列末尾 |
| `drain()` | `fn drain(&mut self) -> Vec<QueuedRecording>` | 取出所有待处理录音（FIFO 顺序），清空队列 |
| `len()` | `fn len(&self) -> usize` | 队列中的录音数量 |
| `is_empty()` | `fn is_empty(&self) -> bool` | 队列是否为空 |

### RetryPolicy

| 方法 | 签名 | 说明 |
|------|------|------|
| `default()` | `fn default() -> Self` | 默认策略：1 次重试，初始延迟 3s，退避因子 2.0 |

### execute_with_retry

```rust
/// Execute an async operation with retries according to the given policy.
///
/// The sleep between retries is interruptible by the cancellation token.
pub async fn execute_with_retry<F, Fut, T, E>(
    policy: &RetryPolicy,
    cancel_token: &CancellationToken,
    operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
```

### NetworkMonitor

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(check_url: String) -> Self` | 创建监控器，默认 30s 间隔、5s 超时 |
| `start()` | `fn start(&self, event_tx: broadcast::Sender<PipelineEvent>) -> CancellationToken` | 启动后台 Tokio 任务，返回用于停止的 CancellationToken |

### Recovery

| 函数 | 签名 | 说明 |
|------|------|------|
| `scan_unfinished_recordings()` | `fn scan_unfinished_recordings(cache_dir: &Path) -> Vec<RecoveryInfo>` | 扫描缓存目录中状态为 `"recording"` 的孤立文件 |

---

## 处理流程

```
process_audio(request, cancel_token)
  |
  +-- [检查取消] --> PipelineError::Cancelled
  |
  +-- emit(TranscriptionStarted)
  |
  +-- Stage 1: STT (with retry)
  |     |
  |     +-- 成功 --> emit(TranscriptionComplete { raw_text })
  |     +-- 失败 --> emit(Error { raw_text: None }) --> return Err(PipelineError::Stt)
  |
  +-- [检查取消] --> PipelineError::Cancelled
  |
  +-- emit(ProcessingStarted)
  |
  +-- Stage 2: LLM (with retry)
  |     |
  |     +-- 成功 --> emit(ProcessingComplete { processed_text })
  |     +-- 失败 --> emit(Error { raw_text: Some(raw_text) }) --> return Err(PipelineError::Llm)
  |
  +-- return Ok(processed_text)
```

**关键设计：**
- CancellationToken 在每个 Stage 之前检查
- 重试等待期间 sleep 可被 cancel 中断（`tokio::select!`）
- LLM 失败时 Error 事件携带 `raw_text`，让 UI 可以提供「插入原始转录」的降级选项
- STT 失败时 Error 事件的 `raw_text` 为 `None`

---

## 错误处理策略

### PipelineError 用户操作映射

```rust
impl PipelineError {
    pub fn user_action(&self) -> UserAction {
        match self {
            PipelineError::Audio(AudioError::NoInputDevice | AudioError::PermissionDenied) => {
                UserAction::CheckMicrophone        // → 前往系统设置
            }
            PipelineError::Stt(e) => e.user_action(),  // 委托给 STTError
            PipelineError::Llm(e) => e.user_action(),  // 委托给 LLMError
            _ => UserAction::RetryOrQueue,              // → [重试] [稍后处理]
        }
    }
}
```

### 重试策略

- 默认：最多 1 次重试（共 2 次尝试），初始延迟 3 秒，退避因子 2.0
- 延迟间 sleep 可被 `CancellationToken` 中断
- 取消时返回最后一次的错误（非 `PipelineError::Cancelled`）

### 网络监控

- 后台 Tokio 任务每 30 秒发起 HTTP HEAD 请求
- 仅在状态**变化**时发送 `NetworkStatusChanged` 事件（避免重复通知）
- HTTP 超时 5 秒
- 通过 `CancellationToken` 停止后台任务

### 崩溃恢复

- `scan_unfinished_recordings()` 扫描缓存目录中 sidecar 状态为 `"recording"` 的文件
- 查找 `.wav` 文件对应的 `.json` sidecar（注意：使用 `.with_extension("json")` 而非 `.wav.json`）
- 无 sidecar 的音频文件被忽略
- 非 `.wav` 文件被忽略
- 目录不存在时返回空列表（不报错）

---

## 测试覆盖

共 **约 26 个** 单元测试：

### Orchestrator 测试（7 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_happy_path` | STT + LLM 全流程成功 |
| `test_stt_failure_emits_error` | STT 失败时发送 Error 事件（raw_text=None） |
| `test_llm_failure_includes_raw_text` | LLM 失败时 Error 事件携带 raw_text |
| `test_cancellation_before_stt` | STT 开始前取消 -> Cancelled |
| `test_processing_request_with_translate` | Translate 模式完整流程 |
| `test_subscribe` | 订阅事件通道编译和运行验证 |
| MockSTT / MockLLM / FailingSTT / FailingLLM | 测试用 mock provider 实现 |

### Queue 测试（5 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `new_queue_is_empty` | 新队列为空 |
| `enqueue_increases_length` | enqueue 增加长度 |
| `drain_returns_all_items_in_order` | drain 按 FIFO 顺序返回并清空 |
| `drain_on_empty_queue_returns_empty_vec` | 空队列 drain 返回空 Vec |
| `enqueue_after_drain_works` | drain 后可继续 enqueue |

### Retry 测试（5 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_succeeds_first_try` | 首次成功不重试 |
| `test_retries_then_succeeds` | 失败两次后第三次成功 |
| `test_exhausts_retries` | 耗尽重试次数返回最后错误 |
| `test_cancellation_stops_retries` | 取消中断重试循环 |
| `test_default_policy` | 默认策略参数验证 |

### Recovery 测试（5 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_scan_empty_dir` | 空目录返回空结果 |
| `test_scan_nonexistent_dir` | 不存在的目录返回空结果 |
| `test_scan_finds_unfinished` | 识别 status="recording" 的文件，忽略 "done" |
| `test_scan_ignores_audio_without_sidecar` | 无 sidecar 的 WAV 被忽略 |
| `test_scan_ignores_non_wav_files` | 非 WAV 文件被忽略 |

### Network 测试（3 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `emits_online_when_server_is_up` | wiremock 模拟在线状态 |
| `emits_offline_when_server_is_down` | 连接拒绝时发送离线事件 |
| `does_not_emit_duplicate_events` | 状态未变化时不重复发送事件 |

---

## 已知限制

1. **内存离线队列** -- `OfflineQueue` 为纯内存 `Vec<QueuedRecording>`，应用崩溃或重启时队列内容丢失。计划在 Phase 4 Step 4 替换为 SQLite 持久化
2. **硬编码重试策略** -- `RetryPolicy::default()` 的参数（1 次重试、3s 延迟、2.0 退避）固定在代码中，不可通过配置文件调整
3. **无流式处理** -- STT 和 LLM 均为一次性调用，不支持流式转录或 SSE 输出
4. **Recovery sidecar 路径不一致** -- `recovery.rs` 使用 `.with_extension("json")`（即 `session1.json`），而 `cache.rs` 使用 `.wav.json` 后缀（即 `test.wav.json`），两种命名约定在同一项目中共存
5. **网络监控 URL 未配置化** -- `NetworkMonitor::new()` 接受 URL 参数但未提供默认的检查端点
6. **broadcast channel 容量** -- 事件广播使用 `broadcast::channel(32)`（在测试中），实际生产环境下的容量配置需要评估
7. **Pipeline 无状态追踪** -- `Pipeline` 不追踪当前是否正在处理，`PipelineError::Busy` 定义但未在编排器中使用
8. **无取消后清理** -- 取消 pipeline 后不自动清理已上传或中间状态的资源
