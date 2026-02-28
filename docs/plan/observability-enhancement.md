# 全链路可观测性增强计划

## Context

TingYuXuan 在 Windows 上开发调试时，缺乏端到端的可观测性。当前状况：

| 层 | 现状 | 问题 |
|----|------|------|
| **Rust Tauri 层** | 有 `tracing` 基础设施，22 个 async 命令中 0 个使用 `#[instrument]` | `build_pipeline` 静默失败、快捷键无日志、事件桥接无日志 |
| **Rust Core 库** | 部分模块有 warn/error 日志 | session 生命周期无日志、LLM 无耗时跟踪、STT WebSocket 缺生命周期日志 |
| **React 前端** | 整个应用仅 5 个 `console.*` 调用 | 事件静默消费、状态变更无追踪、无结构化日志工具 |
| **跨端关联** | session_id 在 Rust 生成 | 前端日志不携带 session_id，无法关联完整链路 |

**目标：** 遵循 `tracing` 最佳实践，让录音全链路（快捷键 → 录音 → STT → LLM → 注入）在终端和 DevTools 中完整可追踪。

---

## 启动开发环境

```bash
npm install
RUST_LOG=tingyuxuan=debug npx tauri dev
```

终端看 Rust 日志，F12 DevTools Console 看前端日志。

---

## Step 1: 前端 — 带 session 上下文的 tagged logger

### 1a. 新建 `src/utils/logger.ts`

设计原则：
- **Tagged logger 模式** — 每个模块创建 tagged logger 实例，DevTools 过滤 `TYX:FloatingBar` 精准定位
- **Session 上下文** — 活跃 session 期间自动附带 session_id 前 8 位
- **DEV-only debug** — production 只输出 warn+，debug/info 被编译时优化
- **零依赖** — 纯 console API 包装

```typescript
type Level = "debug" | "info" | "warn" | "error";

const LEVEL_VALUE: Record<Level, number> = { debug: 0, info: 1, warn: 2, error: 3 };
const LEVEL_FN: Record<Level, "debug" | "info" | "warn" | "error"> = {
  debug: "debug", info: "info", warn: "warn", error: "error",
};
const threshold = LEVEL_VALUE[import.meta.env.DEV ? "debug" : "warn"];

let _sessionId: string | null = null;
export function setLogSession(id: string | null) { _sessionId = id; }

function emit(level: Level, tag: string, msg: string, data?: unknown) {
  if (LEVEL_VALUE[level] < threshold) return;
  const prefix = _sessionId
    ? `[TYX:${tag}:${_sessionId.slice(0, 8)}]`
    : `[TYX:${tag}]`;
  const fn = console[LEVEL_FN[level]];
  data !== undefined ? fn(prefix, msg, data) : fn(prefix, msg);
}

export function createLogger(tag: string) {
  return {
    debug: (msg: string, data?: unknown) => emit("debug", tag, msg, data),
    info:  (msg: string, data?: unknown) => emit("info",  tag, msg, data),
    warn:  (msg: string, data?: unknown) => emit("warn",  tag, msg, data),
    error: (msg: string, data?: unknown) => emit("error", tag, msg, data),
  };
}
```

### 1b. 修改 `src/stores/appStore.ts`

在 `reset()` action 中清除 session 上下文：

```typescript
import { setLogSession } from "../utils/logger";

reset: () => {
  setLogSession(null);
  set({ /* existing reset fields */ });
}
```

### 1c. 修改 `src/components/FloatingBar.tsx`

pipeline-event listener 和 shortcut-action listener 添加日志：

```typescript
import { createLogger, setLogSession } from "../utils/logger";
const log = createLogger("FloatingBar");

// pipeline-event listener — 每个事件记 debug 日志
log.debug(`pipeline-event: ${data.type}`, data);
// RecordingStarted 时绑定 session 上下文:
setLogSession(data.session_id);

// shortcut-action listener 入口:
log.debug(`shortcut: ${action}`, { currentState });

// handleCancel / handleConfirm / handleRetry 入口:
log.debug("user action: cancel/confirm/retry");
```

### 1d. 修改 `src/components/ErrorBoundary.tsx`

```typescript
import { createLogger } from "../utils/logger";
const log = createLogger("ErrorBoundary");

componentDidCatch(error: Error, info: ErrorInfo) {
  log.error("React render error", {
    message: error.message,
    stack: info.componentStack,
  });
}
```

---

## Step 2: Rust Tauri 层 — `#[instrument]` 全覆盖 + 关键路径补盲

### 2a. `src-tauri/src/commands.rs`

**Pipeline 构建可观测性（当前完全静默）：**

```rust
pub fn build_pipeline(config: &AppConfig, event_tx: &...) -> Option<Arc<Pipeline>> {
    let stt_key = resolve_api_key("stt", &config.stt.api_key_ref);
    if stt_key.is_none() {
        tracing::warn!("Pipeline build skipped: no STT API key");
        return None;
    }
    let stt_key = stt_key?;

    let llm_key = resolve_api_key("llm", &config.llm.api_key_ref);
    if llm_key.is_none() {
        tracing::warn!("Pipeline build skipped: no LLM API key");
        return None;
    }
    let llm_key = llm_key?;

    let stt_provider = match stt::create_streaming_stt_provider(&config.stt, stt_key) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create STT provider");
            return None;
        }
    };
    // LLM provider 同理...

    tracing::info!("Pipeline built successfully");
    Some(Arc::new(Pipeline::new(...)))
}
```

**录音命令 `#[instrument]`：**

```rust
#[tauri::command]
#[tracing::instrument(skip_all, fields(mode))]
pub async fn start_recording(mode: String, ...) -> Result<String, String> {
    // 生成 session_id 后动态记录到 span
    tracing::Span::current().record("session_id", &session_id.as_str());
    // ...existing code...
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stop_recording(...) -> Result<String, String> { ... }

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn cancel_recording(...) -> Result<(), String> { ... }
```

**连接测试（当前零日志）：**

```rust
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn test_stt_connection(...) -> Result<bool, String> {
    tracing::info!("Testing STT connection");
    let result = provider.test_connection().await;
    match &result {
        Ok(true) => tracing::info!("STT connection test passed"),
        Ok(false) => tracing::warn!("STT connection test: returned false"),
        Err(e) => tracing::warn!(error = %e, "STT connection test failed"),
    }
    result.map_err(|e| e.to_string())
}
// test_llm_connection 同理
```

**配置/字典/历史命令 — 仅加 `#[instrument]`：**

```rust
#[tracing::instrument(skip_all)]
pub async fn save_config(...) { ... }

#[tracing::instrument(skip_all, fields(limit))]
pub async fn get_recent_history(...) { ... }

// inject_text, save_api_key, get_api_key, search_history,
// delete_history, clear_history, get/add/remove_dictionary_word,
// check_platform_permissions, open_permission_settings 等同理
```

### 2b. `src-tauri/src/lib.rs`

**快捷键处理：**

```rust
async fn handle_shortcut_action(handle: &tauri::AppHandle, action: &str) {
    tracing::debug!(action, "Shortcut triggered");
    match action {
        mode @ ("dictate" | "translate" | "ai_assistant") => {
            let is_recording = recorder.0.is_recording().await;
            tracing::debug!(mode, is_recording, "Toggle recording");
            // ...
```

**事件桥接（跳过高频 VolumeUpdate）：**

```rust
if !matches!(&event, PipelineEvent::VolumeUpdate { .. }) {
    tracing::debug!(?event, "Forwarding event to frontend");
}
```

### 2c. `src-tauri/src/recorder_actor.rs`

```rust
fn handle_command(cmd: RecorderCommand, recorder: &mut AudioRecorder) {
    match cmd {
        RecorderCommand::Start { reply } => {
            tracing::debug!("Recorder: start requested");
            let result = recorder.start().map_err(|e| e.to_string());
            match &result {
                Ok(_) => tracing::info!("Recording started"),
                Err(e) => tracing::error!(error = %e, "Recording start failed"),
            }
            // ...send reply...
        }
        RecorderCommand::Stop { reply } => {
            tracing::debug!("Recorder: stop requested");
            // ...
        }
        RecorderCommand::Cancel { reply } => {
            tracing::debug!("Recorder: cancel requested");
            // ...
        }
        RecorderCommand::IsRecording { .. } => { /* 无需日志 */ }
    }
}
```

---

## Step 3: Rust Core 库 — 管线生命周期可观测性

### 3a. `crates/tingyuxuan-core/src/pipeline/session.rs`

```rust
impl SessionOrchestrator {
    pub async fn start(pipeline: &Pipeline, config: SessionConfig) -> Result<ManagedSession, PipelineError> {
        tracing::info!(mode = %config.mode, "Starting streaming session");
        let streaming_session = pipeline.start_streaming(&config.stt_options).await?;
        tracing::debug!("STT WebSocket connected, session ready");
        Ok(ManagedSession { ... })
    }

    pub async fn finish(pipeline: &Pipeline, mut session: ManagedSession) -> SessionResult {
        if session.cancel_token.is_cancelled() {
            tracing::info!("Session already cancelled before finish");
            return SessionResult::Cancelled;
        }
        tracing::debug!("Closing audio stream, collecting STT results");
        drop(session.audio_tx);

        let collect_result = Self::collect_stt_results(event_rx, &session.cancel_token).await;
        match &collect_result {
            Ok(text) => tracing::debug!(len = text.len(), "STT transcript collected"),
            Err(e) => tracing::warn!(error = %e, "STT collection failed"),
        }
        // ...LLM processing...
    }

    async fn collect_stt_results(...) -> Result<String, PipelineError> {
        // Final 事件处:
        Some(StreamingSTTEvent::Final { text, sentence_index }) => {
            tracing::debug!(sentence_index, len = text.len(), "STT sentence final");
            finals.push((sentence_index, text));
        }
        // 超时处:
        _ = &mut timeout => {
            tracing::warn!(timeout_secs = STT_COLLECT_TIMEOUT_SECS, "STT collection timed out");
            return Err(...);
        }
    }
}
```

### 3b. `crates/tingyuxuan-core/src/pipeline/orchestrator.rs`

**LLM 处理耗时跟踪：**

```rust
pub async fn process_transcript(&self, raw_text: String, request: &ProcessingRequest, ...) -> Result<String, PipelineError> {
    tracing::debug!(mode = %request.mode, raw_len = raw_text.len(), "Starting LLM processing");
    let start = std::time::Instant::now();
    // ...existing LLM call...
    let elapsed = start.elapsed();
    tracing::info!(duration_ms = elapsed.as_millis() as u64, "LLM processing complete");
    Ok(processed)
}
```

### 3c. `crates/tingyuxuan-core/src/llm/openai_compat.rs`

**HTTP 请求/响应/错误日志：**

```rust
async fn send_request(&self, messages: Vec<ChatMessage>) -> Result<ChatCompletionResponse, LLMError> {
    let url = self.completions_url();
    tracing::debug!(model = %self.model, url = %url, "Sending LLM request");

    let response = self.client.post(url)...send().await
        .map_err(|e| {
            if e.is_timeout() {
                tracing::warn!("LLM request timed out");
                LLMError::Timeout
            } else {
                tracing::error!(error = %e, "LLM network error");
                LLMError::NetworkError(e.to_string())
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        let status_code = status.as_u16() as u32;
        let body_text = response.text().await...;
        tracing::warn!(status = status_code, body = %body_text, "LLM API error");
        return Err(match status_code { ... });
    }

    let resp = response.json::<ChatCompletionResponse>().await...;
    if let Some(usage) = &resp.usage {
        tracing::debug!(tokens = usage.total_tokens, "LLM response received");
    }
    Ok(resp)
}
```

### 3d. `crates/tingyuxuan-core/src/stt/dashscope_streaming.rs`

**WebSocket 生命周期日志：**

```rust
fn start_stream<'a>(...) -> Pin<Box<...>> {
    Box::pin(async move {
        let task_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(task_id = %task_id, model = %self.model, "Starting STT stream");

        let (ws_stream, _) = tokio_tungstenite::connect_async_tls_with_config(...)
            .await.map_err(|e| {
                tracing::error!(error = %e, "STT WebSocket connection failed");
                STTError::NetworkError(...)
            })?;

        tracing::debug!(task_id = %task_id, "STT WebSocket connected, sending StartTranscription");
        // ...send start + wait_for_started...
        tracing::debug!(task_id = %task_id, "STT transcription started");
        Ok(StreamingSession { audio_tx, event_rx })
    })
}
```

### 3e. `crates/tingyuxuan-core/src/audio/recorder.rs`

**设备信息 + 采样计数（仅 start/stop，不在 hot path）：**

```rust
pub fn new() -> Result<Self, AudioError> {
    if mock_mode {
        tracing::info!("AudioRecorder initialized in mock mode");
    } else {
        let device = host.default_input_device()...;
        let device_name = device.name().unwrap_or_else(|_| "<unknown>".into());
        tracing::info!(device = %device_name, "AudioRecorder initialized");
    }
    Ok(...)
}

pub fn start(&mut self) -> Result<mpsc::Receiver<AudioChunk>, AudioError> {
    tracing::debug!(mock = self.mock_mode, "Starting audio capture");
    // ...
}

pub fn stop(&mut self) -> Result<(), AudioError> {
    let sample_count = self.inner.lock()...sample_count;
    tracing::info!(samples = sample_count, "Recording stopped");
    // ...
}
```

### 3f. `crates/tingyuxuan-core/src/pipeline/retry.rs`

**首次成功路径日志（当前静默）：**

```rust
Ok(value) => {
    if attempt > 0 {
        tracing::info!(attempt = attempt + 1, "Operation succeeded after retry");
    }
    return Ok(value);
}
```

---

## 文件清单

| 文件 | 操作 | 要点 |
|------|------|------|
| `src/utils/logger.ts` | **新建** | Tagged logger + session context |
| `src/stores/appStore.ts` | 修改 | reset 清除 session context |
| `src/components/FloatingBar.tsx` | 修改 | 事件/快捷键/用户交互日志 |
| `src/components/ErrorBoundary.tsx` | 修改 | 结构化错误日志 |
| `src-tauri/src/commands.rs` | 修改 | `#[instrument]` 全覆盖 + build_pipeline 补盲 |
| `src-tauri/src/lib.rs` | 修改 | 快捷键 debug + 事件桥接 debug |
| `src-tauri/src/recorder_actor.rs` | 修改 | 命令处理 debug |
| `crates/tingyuxuan-core/src/pipeline/session.rs` | 修改 | session 生命周期 info/debug |
| `crates/tingyuxuan-core/src/pipeline/orchestrator.rs` | 修改 | LLM 耗时跟踪 |
| `crates/tingyuxuan-core/src/llm/openai_compat.rs` | 修改 | HTTP 请求/响应/错误日志 |
| `crates/tingyuxuan-core/src/stt/dashscope_streaming.rs` | 修改 | WebSocket 生命周期日志 |
| `crates/tingyuxuan-core/src/audio/recorder.rs` | 修改 | 设备信息 + 采样计数 |
| `crates/tingyuxuan-core/src/pipeline/retry.rs` | 修改 | 重试成功路径日志 |

## 日志级别约定

| 级别 | 用途 | 示例 |
|------|------|------|
| `error` | 不可恢复的失败 | WebSocket 断开、provider 创建失败、LLM 网络错误 |
| `warn` | 可恢复的异常 | API key 缺失、连接测试失败、超时、重试 |
| `info` | 生命周期节点 | Pipeline built、Recording started/stopped、LLM complete |
| `debug` | 流程细节 | 快捷键触发、事件转发、STT sentence final、请求参数 |

**刻意不加日志的 hot path：**
- `process_mono_f32`（cpal 音频回调，~30fps）
- `VolumeUpdate` 事件转发
- `get_volume_levels`

## 验证方式

1. **启动验证：** `RUST_LOG=tingyuxuan=debug npx tauri dev` → 终端看到 `AudioRecorder initialized` + `Pipeline built successfully`（或 warn 级 skip 原因）
2. **录音链路：** 按 RAlt → 终端完整链路：`Shortcut triggered` → `Toggle recording` → `Recorder: start` → `Starting STT stream` → `STT WebSocket connected`
3. **前端日志：** F12 Console 过滤 `TYX` → 看到 `pipeline-event: RecordingStarted` 带 session_id 前缀
4. **处理链路：** 松开 RAlt → 终端：`Recording stopped` → `STT transcript collected` → `LLM processing complete (duration_ms=xxx)`
5. **取消流程：** Esc → 两端都有取消日志
6. **连接测试：** 设置页 → 测试连接 → `Testing STT/LLM connection` + 结果
7. **回归测试：** `cargo test -p tingyuxuan-core` + `npm test` 全部通过
