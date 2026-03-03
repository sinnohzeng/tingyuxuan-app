# Pipeline 模块（处理流水线）

## 模块职责

Pipeline 模块负责协调单步多模态处理流水线：音频编码 → 多模态 LLM → 文本输出。提供重试、取消、事件广播、网络监控等功能。作为核心调度层，衔接音频缓冲区与最终文本输出之间的异步处理。

---

## 核心类型定义

### Pipeline（编排器）

```rust
/// 管线编排器 — 单步多模态处理（音频编码 → LLM → 文本）。
pub struct Pipeline {
    llm: Box<dyn LLMProvider>,
    event_tx: broadcast::Sender<PipelineEvent>,
    retry_policy: RetryPolicy,
}
```

> **与旧版的区别：** 旧版 `Pipeline` 持有 `stt: Box<dyn STTProvider>` 和 `llm: Box<dyn LLMProvider>` 两个 provider，执行 STT → LLM 两步流水线。新版仅持有一个 `llm`（多模态 provider），一步完成识别和处理。

### ProcessingRequest

```rust
/// 处理请求 — 描述一次多模态处理的参数。
pub struct ProcessingRequest {
    pub mode: ProcessingMode,
    pub context: InputContext,
    pub target_language: Option<String>,
    pub user_dictionary: Vec<String>,
}
```

> **与旧版的区别：** 旧版包含 `audio_path: PathBuf`（音频文件路径），新版不含音频路径，音频通过 `AudioBuffer` 直接传入 `process_audio()`。

### SessionResult

```rust
/// Session 处理结果。
pub enum SessionResult {
    Success { processed_text: String },
    EmptyAudio,
    Failed { error: PipelineError },
    Cancelled,
}
```

### PipelineEvent

```rust
/// Events emitted by the pipeline as it progresses through each stage.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PipelineEvent {
    RecorderStarting { mode: String },
    RecordingStarted { session_id: String, mode: String },
    VolumeUpdate { levels: Vec<f32> },
    RecordingStopped { duration_ms: u64 },
    ThinkingStarted,                         // 音频编码 + LLM 开始
    ProcessingStarted,                       // 兼容旧客户端的别名事件
    ProcessingComplete { processed_text: String },
    Error { message: String, user_action: UserAction },
    NetworkStatusChanged { online: bool },
    RecordingCancelled,
}
```

> **已移除的事件：** `TranscriptionStarted`、`TranscriptionComplete`、`QueuedForLater` — STT 阶段已不存在，离线队列暂未在新管线中启用。
>
> **Error 事件变化：** 不再携带 `raw_text: Option<String>` — 没有中间转录文本可供降级使用。

### RetryPolicy

```rust
/// Configuration for retry behaviour with exponential back-off.
pub struct RetryPolicy {
    pub max_retries: u32,          // 默认: 1
    pub initial_delay_ms: u64,     // 默认: 3000
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

### PipelineError

```rust
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),
    #[error("Pipeline cancelled by user")]
    Cancelled,
    #[error("Pipeline is busy")]
    Busy,
}
```

> **已移除：** `Stt(#[from] STTError)` — STT 模块已删除。

---

## 公开 API

### Pipeline（编排器）

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(llm: Box<dyn LLMProvider>, event_tx: broadcast::Sender<PipelineEvent>) -> Self` | 创建 Pipeline 实例，使用默认 RetryPolicy |
| `subscribe()` | `fn subscribe(&self) -> broadcast::Receiver<PipelineEvent>` | 订阅管线事件 |
| `process_audio()` | `async fn process_audio(&self, buffer: AudioBuffer, request: &ProcessingRequest, cancel_token: CancellationToken) -> Result<String, PipelineError>` | 单步处理：编码音频 → 调用多模态 LLM → 返回处理后文本 |

### execute_with_retry

```rust
/// Execute an async operation with retries according to the given policy.
pub async fn execute_with_retry<F, Fut, T, E>(
    policy: &RetryPolicy,
    cancel_token: &CancellationToken,
    operation: F,
) -> Result<T, E>
```

### NetworkMonitor

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(check_url: String) -> Self` | 创建监控器，默认 30s 间隔、5s 超时 |
| `start()` | `fn start(&self, event_tx: broadcast::Sender<PipelineEvent>) -> CancellationToken` | 启动后台 Tokio 任务，返回用于停止的 CancellationToken |

---

## 处理流程

```
process_audio(buffer, request, cancel_token)
  |
  +-- [检查取消] --> PipelineError::Cancelled
  |
  +-- 编码音频: buffer.encode(Mp3) --失败--> buffer.encode(Wav)
  |
  +-- 构建 ProcessingInput { mode, audio, context, ... }
  |
  +-- emit(ThinkingStarted)
  |
  +-- 调用 LLM (with retry + cancel)
  |     |
  |     +-- 成功 --> 质量闸门校验（空文本/模板占位词拦截） --> emit(ProcessingComplete)
  |     +-- 失败 --> emit(Error { message, user_action }) --> return Err(PipelineError::Llm)
  |
  +-- return Ok(processed_text)
```

**关键设计：**
- 单步处理：没有中间 STT 阶段，一次 API 调用完成识别+润色
- 压缩优先：默认 MP3，编码失败回退 WAV，接口错误时再回退 WAV 重试一次
- `AudioBuffer` 所有权转移到 `process_audio()`，处理后自动释放内存
- CancellationToken 在处理开始前和 LLM 调用期间检查
- 重试等待期间 sleep 可被 cancel 中断（`tokio::select!`）
- 没有 `raw_text` 降级选项 — 如果多模态 LLM 失败，只能重试或取消

---

## 错误处理策略

### PipelineError 用户操作映射

```rust
impl PipelineError {
    pub fn user_action(&self) -> UserAction {
        match self {
            PipelineError::Audio(AudioError::NoInputDevice | AudioError::PermissionDenied) => {
                UserAction::CheckMicrophone
            }
            PipelineError::Llm(e) => e.user_action(),
            _ => UserAction::RetryOrCancel,
        }
    }
}
```

### 重试策略

- 默认：最多 1 次重试（共 2 次尝试），初始延迟 3 秒，退避因子 2.0
- 延迟间 sleep 可被 `CancellationToken` 中断

### 网络监控

- 后台 Tokio 任务每 30 秒发起 HTTP HEAD 请求
- 仅在状态**变化**时发送 `NetworkStatusChanged` 事件
- HTTP 超时 5 秒

---

## 测试覆盖

共 **约 20 个** 单元测试：

### Orchestrator 测试（6 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_process_audio_success` | AudioBuffer → LLM 全流程成功 |
| `test_process_audio_llm_failure` | LLM 失败时发送 Error 事件 |
| `test_cancellation_before_processing` | 处理开始前取消 → Cancelled |
| `test_processing_events_emitted` | ThinkingStarted + ProcessingComplete 事件发射验证 |
| `test_placeholder_transcript_rejected` | 模板占位文案被质量闸门拒绝并发出 Error |
| `test_retry_with_wav_after_mp3_rejection` | MP3 不兼容时自动回退 WAV 再试 |
| `test_translate_mode` | Translate 模式完整流程 |
| `test_subscribe` | 订阅事件通道验证 |

### Retry 测试（5 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_succeeds_first_try` | 首次成功不重试 |
| `test_retries_then_succeeds` | 失败后重试成功 |
| `test_exhausts_retries` | 耗尽重试次数返回最后错误 |
| `test_cancellation_stops_retries` | 取消中断重试循环 |
| `test_default_policy` | 默认策略参数验证 |

### Network 测试（3 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `emits_online_when_server_is_up` | wiremock 模拟在线状态 |
| `emits_offline_when_server_is_down` | 连接拒绝时发送离线事件 |
| `does_not_emit_duplicate_events` | 状态未变化时不重复发送 |

---

## 已知限制

1. **硬编码重试策略** -- `RetryPolicy::default()` 的参数固定在代码中，不可通过配置文件调整
2. **无 LLM 失败降级** -- 旧版可在 LLM 失败时提供原始转录文本，新版多模态管线无中间结果可降级
3. **Pipeline 无状态追踪** -- `Pipeline` 不追踪当前是否正在处理，`PipelineError::Busy` 定义但未使用
4. **网络监控 URL 未配置化** -- `NetworkMonitor::new()` 接受 URL 参数但未提供默认检查端点
5. **broadcast channel 容量** -- 事件广播使用 `broadcast::channel(32)`（测试中），生产环境容量需评估
