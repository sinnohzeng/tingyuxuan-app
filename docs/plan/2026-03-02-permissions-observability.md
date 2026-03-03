# Sprint 8 计划：全平台权限系统 + 可观测性平台集成

## Context

经过多轮 RAlt 录音调试，暴露了两个系统性缺陷：

1. **权限检测缺失**：麦克风权限从未在任何平台检测过。录音失败时用户只看到含糊的错误 + 错误的 `CheckApiKey` 动作按钮（应为 `CheckMicrophone`）。根因：错误类型在 actor 边界退化为 String，前端无法区分错误种类。
2. **可观测性不足**：关键路径日志缺失导致排障极其困难。recorder.start() 失败无日志、LLM TTFB/总耗时无记录、前端用户操作无追踪、无崩溃收集平台。

本计划分三条工作线：**A. 错误类型治理** → **B. 权限 UX** → **C. 可观测性平台**。

### 文档同步要求

- 计划文件持久化到 `docs/plans/2026-03-02-permissions-observability.md`
- 遵循 DDD（文档驱动开发）：实现过程中同步更新 CLAUDE.md、相关文档
- 遵循 SSOT（唯一真值）：权限系统和可观测性的权威文档是本计划文件，其他文档引用不重复
- 实施完成后更新 MEMORY.md（架构决策、新依赖、环境变量）

---

## 架构改进：与行业最佳实践对齐

### 1. 结构化错误传递（消除字符串匹配）

**现状问题**：`RecorderHandle::start()` 返回 `Result<(), String>`，丢失了 `AudioError` 类型信息。前端收到 `invoke("start_recording")` 的错误是一个裸字符串，只能用 `err.includes("配置")` 做脆弱匹配。

**行业惯例**：错误类型在进程边界序列化为结构化 JSON，不退化为裸字符串。

**修复**：`start_recording` 命令返回 `Result<String, StructuredError>`，利用已有的 `StructuredError` 类型（`error.rs:135`）。前端根据 `error_code` 和 `user_action` 字段做分支，不做字符串匹配。

### 2. Telemetry 架构：trait 抽象 + 可替换后端

```
TelemetryBackend trait
├── SentryBackend      — 崩溃/错误上报
├── SlsBackend         — 结构化事件分析
└── CompositBackend    — 组合多个后端
```

事件定义与传输解耦。前端通过 Tauri command `report_event` 上报事件到 Rust，由 Rust 统一分发。避免 SLS 凭证暴露在 JS 端。

### 3. SLS 用 Web Tracking API（简化 90%）

**放弃**：PutLogs API（需要 protobuf 编码 + HMAC-SHA1 签名 + 8 个自定义 header）。
**采用**：Web Tracking API — 匿名 JSON POST，零认证，只需在 SLS 控制台开启 Web Tracking 功能。

```
POST https://{project}.{region}.log.aliyuncs.com/logstores/{logstore}/track
Content-Type: application/json

{"__topic__":"app_event","data":[{"event_type":"session_completed",...}]}
```

限制：3MB/请求、4096 条/请求。对桌面应用绰绰有余。

### 4. 生产级 Sentry 配置

- Source maps：`@sentry/vite-plugin` 构建时自动上传
- Debug symbols：CI release workflow 中 `sentry-cli debug-files upload`
- Release tracking：`sentry::release_name!()` + `sentry-cli releases set-commits --auto`

---

## 工作线 A：错误类型治理

### A1. RecorderHandle 返回类型修复

**文件**：`src-tauri/src/recorder_actor.rs`

将 `RecorderCommand::Start` 的 oneshot 回复类型从 `Result<(), String>` 改为 `Result<(), AudioError>`。AudioError 已实现 `Debug + Display + Error`，序列化由 Tauri 命令层处理。

```rust
enum RecorderCommand {
    Start(oneshot::Sender<Result<(), AudioError>>),
    // ...
}
```

Actor 处理中不再 `.map_err(|e| e.to_string())`，直接传递原始 AudioError。

### A2. start_recording 返回结构化错误

**文件**：`src-tauri/src/commands.rs`

改造 `start_recording` 的错误路径，利用已有的 `StructuredError`：

```rust
#[tauri::command]
pub async fn start_recording(mode: String, ...) -> Result<String, String> {
    // pipeline 不存在 → API key 问题
    let pipeline = pipeline_state.0.read().await.clone()
        .ok_or_else(|| serde_json::to_string(&StructuredError {
            error_code: "not_configured".into(),
            message: "请先在设置中配置 LLM 的 API Key".into(),
            user_action: UserAction::CheckApiKey,
        }).unwrap())?;

    // recorder 启动失败 → 麦克风/设备问题
    if let Err(audio_err) = recorder_state.0.start().await {
        let se = StructuredError::from(&PipelineError::Audio(audio_err));
        return Err(serde_json::to_string(&se).unwrap());
    }
    // ...
}
```

### A3. 前端解析结构化错误

**文件**：`src/features/recording/FloatingBar.tsx`

```typescript
.catch((errStr: string) => {
    try {
        const se = JSON.parse(errStr) as { error_code: string; message: string; user_action: UserAction };
        store.setError(se.message, se.user_action);
    } catch {
        store.setError(errStr, "Retry");
    }
});
```

不再用 `err.includes("配置")` 做字符串匹配。

---

## 工作线 B：全平台权限检测 + UX

### B1. PermissionReport 类型

**文件**：`src-tauri/src/platform/mod.rs`（行 131 附近，保留旧 PermissionStatus 兼容）

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionReport {
    pub all_granted: bool,
    pub microphone: PermissionState,
    pub accessibility: PermissionState,    // macOS only, 其他平台 Granted
    pub input_monitoring: PermissionState, // macOS only, 其他平台 Granted
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState { Granted, Denied, Unknown }
```

### B2. 核心层麦克风探测

**文件**：`crates/tingyuxuan-core/src/audio/recorder.rs`

静态方法，不创建 AudioRecorder 实例：

```rust
pub fn probe_microphone() -> Result<(), AudioError> {
    if mock_mode() { return Ok(()); }
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or(AudioError::NoInputDevice)?;
    device.supported_input_configs()
        .map_err(|_| AudioError::PermissionDenied)?
        .next().ok_or(AudioError::NoInputDevice)?;
    Ok(())
}
```

设计：`default_input_device()` None → NoInputDevice；`supported_input_configs()` 失败 → PermissionDenied（Windows 11 隐私设置关闭麦克风时此处报错）。三平台行为一致。

### B3. 各平台实现

**Windows** (`src-tauri/src/platform/windows.rs`)：
```rust
pub fn check_permissions() -> PermissionReport {
    let mic = match AudioRecorder::probe_microphone() {
        Ok(()) => PermissionState::Granted,
        Err(_) => PermissionState::Denied,
    };
    PermissionReport {
        all_granted: mic == PermissionState::Granted,
        microphone: mic,
        accessibility: PermissionState::Granted,
        input_monitoring: PermissionState::Granted,
    }
}

pub fn open_permission_settings_for(target: Option<&str>) {
    let uri = match target {
        Some("microphone") => "ms-settings:privacy-microphone",
        _ => "ms-settings:privacy-microphone",
    };
    let _ = std::process::Command::new("cmd").args(["/c", "start", uri]).spawn();
}
```

**macOS** (`src-tauri/src/platform/macos.rs`)：
- 扩展现有 `check_permissions()` 返回 `PermissionReport`，增加 mic 字段
- `open_permission_settings_for("microphone")` → `Privacy_Microphone` pane

**Linux** (`src-tauri/src/platform/linux.rs`)：
- cpal 探测 mic，其余 Granted
- `open_permission_settings_for()` → 尝试 `pavucontrol` 或 `gnome-control-center sound`

### B4. Tauri 命令更新

**文件**：`src-tauri/src/commands.rs`

```rust
#[tauri::command]
pub async fn check_platform_permissions() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    let report = crate::platform::macos::check_permissions();
    #[cfg(target_os = "windows")]
    let report = crate::platform::windows::check_permissions();
    #[cfg(target_os = "linux")]
    let report = crate::platform::linux::check_permissions();

    tracing::info!(?report, "Permission check");
    serde_json::to_string(&report).map_err(|e| e.to_string())
}
```

`open_permission_settings` 同理扩展为三平台。

### B5. 前端 PermissionGuide 重写

**文件**：`src/features/onboarding/PermissionGuide.tsx`（130→~170 行）

1. 解析 JSON `PermissionReport`
2. 按 denied 权限动态渲染卡片（microphone 全平台、accessibility/input_monitoring macOS）
3. 窗口获焦自动重检（`getCurrentWindow().onFocusChanged`）
4. `all_granted` 时自动调 `onComplete()`

新增类型到 `src/shared/lib/types.ts`：

```typescript
interface PermissionReport {
  all_granted: boolean;
  microphone: "granted" | "denied" | "unknown";
  accessibility: "granted" | "denied" | "unknown";
  input_monitoring: "granted" | "denied" | "unknown";
}
```

### B6. ErrorPanel 麦克风引导

**文件**：`src/features/recording/ErrorPanel.tsx`

`CheckMicrophone` → 增加"打开麦克风设置"主按钮 + "关闭"次按钮。

`FloatingBar.tsx` 传入回调 `onOpenMicSettings` → `invoke("open_permission_settings", { target: "microphone" })`。

需要扩展 `ErrorPanelProps` 增加 `onOpenMicSettings` 回调。

---

## 工作线 C：可观测性平台集成

### C1. Sentry 集成（SaaS 先行）

**部署策略**：先用 sentry.io SaaS 免费版（5K errors/月）快速跑通。开发者有 VPN 可访问仪表板和 API。产品正式上线前迁移到自托管（只需改 `SENTRY_DSN` 环境变量，代码零改动）。

**依赖**（`src-tauri/Cargo.toml`）：

```toml
sentry = "0.42"
tauri-plugin-sentry = "0.5"
```

**Capabilities**（`src-tauri/capabilities/default.json`）：增加 `"sentry:default"`

**初始化**（`src-tauri/src/lib.rs`）：

提取 `init_sentry()` 辅助函数，DSN 从环境变量读取：

```rust
fn init_sentry() -> sentry::ClientInitGuard {
    let dsn = std::env::var("SENTRY_DSN").unwrap_or_default();
    sentry::init((dsn, sentry::ClientOptions {
        release: sentry::release_name!(),
        environment: Some(if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        }.into()),
        auto_session_tracking: true,
        sample_rate: 1.0,
        traces_sample_rate: 0.2,
        ..Default::default()
    }))
}
```

`run()` 中调用顺序：`init_tracing()` → `init_sentry()` → `tauri::Builder::default().plugin(tauri_plugin_sentry::init(&client))`。

**前端**：`tauri-plugin-sentry` 自动注入 `@sentry/browser` 到 WebView，零 JS 代码改动。Breadcrumbs 自动合并 Rust + JS 双端。

**SaaS 注册步骤**（一次性）：
1. 访问 sentry.io 注册账号（免费 Developer Plan）
2. 创建 Project（Platform 选 "Other"）
3. 获取 DSN（Settings → Projects → Client Keys）
4. 设置本地环境变量 `SENTRY_DSN`

**Source maps**（`vite.config.ts`）：

```typescript
import { sentryVitePlugin } from "@sentry/vite-plugin";

// 在 plugins 数组末尾添加（必须在最后）
sentryVitePlugin({
  org: process.env.SENTRY_ORG,
  project: process.env.SENTRY_PROJECT,
  authToken: process.env.SENTRY_AUTH_TOKEN,
  url: process.env.SENTRY_URL, // SaaS 可省略此行
  sourcemaps: { filesToDeleteAfterUpload: ["**/*.js.map"] },
})
```

`build.sourcemap: true` 需要在 vite.config.ts 中设置。

**CI 集成**（`.github/workflows/release.yml`）：

在各平台 build job 完成后增加步骤：
1. `sentry-cli releases new` + `set-commits --auto`
2. `sentry-cli sourcemaps upload`（JS source maps）
3. `sentry-cli debug-files upload`（.pdb/.dSYM/ELF）
4. `sentry-cli releases finalize`

### C2. Rust Breadcrumbs

**文件**：`src-tauri/src/commands.rs`

在关键命令中添加结构化 breadcrumbs：

```rust
sentry::add_breadcrumb(sentry::Breadcrumb {
    category: Some("recording".into()),
    message: Some(format!("start: mode={mode}")),
    level: sentry::Level::Info,
    data: {
        let mut m = sentry::protocol::Map::new();
        m.insert("session_id".into(), session_id.clone().into());
        m.insert("has_context".into(), has_selected_text.into());
        m
    },
    ..Default::default()
});
```

添加点：`start_recording`、`stop_recording`、`cancel_recording`、`save_config`、`inject_text`。

### C3. 管线性能埋点

**文件**：`crates/tingyuxuan-core/src/pipeline/orchestrator.rs`

在 `process_audio()` 中增加结构化计时字段（复用现有 tracing span）：

```rust
let encode_start = Instant::now();
let encoded = buffer.encode(AudioFormat::Wav)?;
let encode_ms = encode_start.elapsed().as_millis() as u64;
tracing::info!(encode_ms, bytes = encoded.data.len(), "Audio encoded");

let llm_start = Instant::now();
// ... LLM 调用 ...
let llm_ms = llm_start.elapsed().as_millis() as u64;
tracing::info!(llm_ms, tokens = ?tokens_used, "LLM complete");
```

**文件**：`crates/tingyuxuan-core/src/llm/multimodal.rs`

在 `parse_sse_response()` 中记录 TTFB：

```rust
// 首个 content chunk 到达时
if accumulated.is_empty() && !content.is_empty() {
    tracing::info!(ttfb_ms = start.elapsed().as_millis(), "LLM TTFB");
}
```

### C4. 音频设备日志

**文件**：`crates/tingyuxuan-core/src/audio/recorder.rs`

在 `start_real_stream()` 选定设备后：

```rust
tracing::info!(
    device = %device.name().unwrap_or_else(|_| "unknown".into()),
    sample_rate = config.sample_rate.0,
    channels = config.channels,
    format = ?sample_format,
    "Audio device selected"
);
```

### C5. 文本注入日志

**文件**：`src-tauri/src/commands.rs`

```rust
match injector.inject_text(&processed_text) {
    Ok(()) => tracing::info!(chars = processed_text.chars().count(), "Text injected"),
    Err(e) => tracing::error!(%e, chars = processed_text.chars().count(), "Injection failed"),
}
```

### C6. Telemetry 模块（SLS 传输）

**新目录**：`crates/tingyuxuan-core/src/telemetry/`

```
telemetry/
├── mod.rs          // pub use, TelemetryBackend trait
├── events.rs       // TelemetryEvent enum + Envelope
└── sls.rs          // SLS Web Tracking HTTP 传输
```

**Trait 定义**（`mod.rs`）：

```rust
#[async_trait]
pub trait TelemetryBackend: Send + Sync {
    fn track(&self, event: TelemetryEvent);
    async fn flush(&self);
}
```

**事件定义**（`events.rs`）：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type")]
pub enum TelemetryEvent {
    SessionStarted { session_id: String, mode: String, has_context: bool },
    SessionCompleted {
        session_id: String,
        recording_ms: u64,
        encode_ms: u64,
        llm_ttfb_ms: u64,
        llm_total_ms: u64,
        tokens: Option<u32>,
        result_chars: usize,
        injected: bool,
    },
    SessionFailed { session_id: String, error_code: String, stage: String, duration_ms: u64 },
    SessionCancelled { session_id: String, stage: String, duration_ms: u64 },
    AppStarted { version: String, platform: String, has_api_key: bool, model: String },
    PermissionCheck { platform: String, microphone: String, accessibility: String },
    AudioDeviceInfo { device: String, sample_rate: u32, channels: u16 },
    UserAction { action: String, context: Option<String> },
}
```

**SLS 传输**（`sls.rs`）— 使用 Web Tracking API：

```rust
pub struct SlsTransport {
    client: reqwest::Client,
    endpoint: String,  // https://{project}.{region}.log.aliyuncs.com/logstores/{logstore}/track
    buffer: Arc<Mutex<Vec<TelemetryEvent>>>,
    device_id: String,
    app_version: String,
}
```

- `track()` → 放入 buffer
- 后台任务每 30 秒或 buffer ≥ 50 条时 flush
- Flush = POST JSON 到 Web Tracking endpoint
- 失败静默（日志 warn），不影响应用
- 应用退出时同步 flush 一次

**请求格式**：

```json
{
  "__topic__": "app_event",
  "__source__": "{device_id}",
  "data": [
    {
      "event_type": "session_completed",
      "session_id": "...",
      "timestamp": "2026-03-02T10:00:00Z",
      "app_version": "0.8.1",
      "platform": "windows",
      ...
    }
  ]
}
```

### C7. 前端埋点

**新文件**：`src/shared/lib/telemetry.ts`

```typescript
export async function trackEvent(
  event: string,
  props?: Record<string, unknown>,
): Promise<void> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("report_telemetry_event", {
      event: JSON.stringify({ event_type: event, ...props }),
    });
  } catch {
    // 非 Tauri 环境或未初始化，静默忽略
  }
}
```

前端不直接调 SLS，统一经 Rust 后端汇总上报。

**新 Tauri 命令**（`commands.rs`）：

```rust
#[tauri::command]
pub async fn report_telemetry_event(event: String, telemetry: State<'_, TelemetryState>) -> Result<(), String> {
    if let Ok(evt) = serde_json::from_str::<TelemetryEvent>(&event) {
        telemetry.0.track(evt);
    }
    Ok(())
}
```

**埋点位置**：

| 文件 | 事件 |
|------|------|
| `FloatingBar.tsx` handleCancel | `trackEvent("user_action", { action: "cancel" })` |
| `FloatingBar.tsx` handleConfirm | `trackEvent("user_action", { action: "confirm" })` |
| `FloatingBar.tsx` handleDismiss | `trackEvent("user_action", { action: "dismiss_error" })` |
| `ErrorPanel.tsx` 各按钮 | `trackEvent("user_action", { action: "error_retry/open_settings/..." })` |
| `ResultPanel.tsx` copy | `trackEvent("user_action", { action: "result_copy" })` |
| `ResultPanel.tsx` insert | `trackEvent("user_action", { action: "result_insert" })` |

### C8. MCP 工具

**新目录**：`mcp-servers/`

**Sentry MCP** (`mcp-servers/sentry-mcp.js`)：

Node.js MCP server，封装 Sentry REST API：

| 工具名 | API | 用途 |
|--------|-----|------|
| `sentry_list_issues` | `GET /api/0/projects/{org}/{proj}/issues/?query=...` | 列出近期错误 |
| `sentry_get_issue` | `GET /api/0/issues/{id}/` | 错误详情 + 堆栈 |
| `sentry_get_latest_event` | `GET /api/0/issues/{id}/events/latest/` | 最新事件 + breadcrumbs |
| `sentry_search` | `GET /api/0/projects/{org}/{proj}/issues/?query=...` | 按关键词搜索 |

认证：`SENTRY_AUTH_TOKEN` + `SENTRY_URL` 环境变量。

**SLS MCP** (`mcp-servers/sls-mcp.js`)：

封装 SLS GetLogs API（此处需要 HMAC 签名，但只有 MCP server 端需要，不影响客户端）：

| 工具名 | 用途 |
|--------|------|
| `sls_query` | 执行 SQL 查询（支持 `SELECT ... WHERE event_type = ...`） |
| `sls_get_session` | 按 session_id 查询完整事件链 |
| `sls_error_stats` | 近 24h/7d 错误统计 |
| `sls_performance` | P50/P95 性能指标 |

**.mcp.json 配置**：

```json
{
  "sentry": {
    "type": "stdio",
    "command": "node",
    "args": ["mcp-servers/sentry-mcp.js"],
    "env": {
      "SENTRY_URL": "${SENTRY_URL}",
      "SENTRY_AUTH_TOKEN": "${SENTRY_AUTH_TOKEN}",
      "SENTRY_ORG": "${SENTRY_ORG}",
      "SENTRY_PROJECT": "${SENTRY_PROJECT}"
    }
  },
  "sls": {
    "type": "stdio",
    "command": "node",
    "args": ["mcp-servers/sls-mcp.js"],
    "env": {
      "SLS_ENDPOINT": "${SLS_ENDPOINT}",
      "SLS_PROJECT": "${SLS_PROJECT}",
      "SLS_LOGSTORE": "${SLS_LOGSTORE}",
      "SLS_ACCESS_KEY_ID": "${SLS_ACCESS_KEY_ID}",
      "SLS_ACCESS_KEY_SECRET": "${SLS_ACCESS_KEY_SECRET}"
    }
  }
}
```

### C9. CI/CD 集成

**文件**：`.github/workflows/release.yml`

在各平台 build job 中增加 Sentry release 步骤：

```yaml
- name: Setup Sentry CLI
  uses: getsentry/action-cli-setup@v2
  with:
    token: ${{ secrets.SENTRY_AUTH_TOKEN }}

- name: Sentry Release
  run: |
    VERSION=${{ github.ref_name }}
    sentry-cli releases new -o $SENTRY_ORG -p $SENTRY_PROJECT $VERSION
    sentry-cli releases set-commits $VERSION --auto
    sentry-cli sourcemaps upload --release $VERSION dist/
    sentry-cli debug-files upload src-tauri/target/release/
    sentry-cli releases finalize $VERSION
  env:
    SENTRY_ORG: ${{ secrets.SENTRY_ORG }}
    SENTRY_PROJECT: ${{ secrets.SENTRY_PROJECT }}
    SENTRY_URL: ${{ secrets.SENTRY_URL }}
```

**GitHub Secrets 需配置**：`SENTRY_ORG`、`SENTRY_PROJECT`、`SENTRY_AUTH_TOKEN`、`SENTRY_URL`

**Vite 构建**：`build.sourcemap: true` + `@sentry/vite-plugin` 自动上传。

---

## 修改文件清单

### Rust 后端

| 文件 | 修改内容 | 行数 |
|------|---------|------|
| `crates/tingyuxuan-core/src/audio/recorder.rs` | `probe_microphone()` + 设备日志 | +25 |
| `crates/tingyuxuan-core/src/telemetry/mod.rs` | **新文件**：TelemetryBackend trait | ~25 |
| `crates/tingyuxuan-core/src/telemetry/events.rs` | **新文件**：TelemetryEvent enum | ~80 |
| `crates/tingyuxuan-core/src/telemetry/sls.rs` | **新文件**：SLS Web Tracking 传输 | ~120 |
| `crates/tingyuxuan-core/src/lib.rs` | 添加 `pub mod telemetry` | +1 |
| `crates/tingyuxuan-core/Cargo.toml` | 添加 `async-trait` 依赖 | +1 |
| `src-tauri/Cargo.toml` | sentry + tauri-plugin-sentry 依赖 | +3 |
| `src-tauri/capabilities/default.json` | sentry:default | +1 |
| `src-tauri/src/lib.rs` | `init_sentry()` + plugin 注册 | +20 |
| `src-tauri/src/platform/mod.rs` | PermissionReport + PermissionState | +20 |
| `src-tauri/src/platform/windows.rs` | check_permissions + open_settings | +25 |
| `src-tauri/src/platform/macos.rs` | 扩展 check_permissions + mic | +20 |
| `src-tauri/src/platform/linux.rs` | check_permissions + open_settings | +25 |
| `src-tauri/src/commands.rs` | 结构化错误 + breadcrumbs + report_event + 注入日志 | +60 |
| `src-tauri/src/recorder_actor.rs` | 返回 AudioError 而非 String | ~10 改 |
| `src-tauri/src/state.rs` | TelemetryState managed state | +5 |
| `crates/tingyuxuan-core/src/pipeline/orchestrator.rs` | LLM 计时字段 | +15 |
| `crates/tingyuxuan-core/src/llm/multimodal.rs` | TTFB 日志 | +8 |

### 前端

| 文件 | 修改内容 | 行数 |
|------|---------|------|
| `src/shared/lib/types.ts` | PermissionReport 接口 | +8 |
| `src/shared/lib/telemetry.ts` | **新文件**：trackEvent | ~20 |
| `src/features/onboarding/PermissionGuide.tsx` | 重写：JSON + 全平台 + 自动重检 | 重写 130→~170 |
| `src/features/recording/ErrorPanel.tsx` | 麦克风设置按钮 + trackEvent | +15 |
| `src/features/recording/FloatingBar.tsx` | 结构化错误解析 + trackEvent | +20 |
| `src/features/recording/ResultPanel.tsx` | trackEvent | +5 |
| `vite.config.ts` | sentryVitePlugin + sourcemap | +15 |
| `package.json` | @sentry/vite-plugin devDep | +1 |

### 基础设施

| 文件 | 修改内容 |
|------|---------|
| `mcp-servers/sentry-mcp.js` | **新文件**：~150 行 |
| `mcp-servers/sls-mcp.js` | **新文件**：~150 行 |
| `.mcp.json` | 添加 sentry + sls 服务器 |
| `.github/workflows/release.yml` | Sentry release 步骤 |

**总计**：~700 行新增/修改，5 个新文件

---

## 实施顺序

**阶段 1（Rust 基础，无前端依赖）**：
1. A1: recorder_actor 返回 AudioError
2. B1-B3: PermissionReport 类型 + probe_microphone + 三平台实现
3. A2: commands.rs 结构化错误返回
4. B4: 权限命令更新

**阶段 2（可观测性 Rust 层，可与阶段 1 并行）**：
5. C1: Sentry 依赖 + capabilities + init_sentry
6. C2: Breadcrumbs
7. C3-C5: 管线计时 + 设备日志 + 注入日志
8. C6: telemetry 模块 + SLS transport

**阶段 3（前端，依赖阶段 1+2）**：
9. A3: FloatingBar 结构化错误解析
10. B5-B6: PermissionGuide 重写 + ErrorPanel 麦克风按钮
11. C7: telemetry.ts + 各处 trackEvent
12. C1(前端): vite.config.ts sentryVitePlugin

**阶段 4（基础设施 + 文档）**：
13. C8: MCP 服务器（sentry-mcp + sls-mcp）
14. C9: CI release workflow 更新
15. 文档同步：计划持久化到 docs/plans/ + 更新 CLAUDE.md + 更新 MEMORY.md

---

## 验证步骤

### 权限系统
1. `cargo check --manifest-path src-tauri/Cargo.toml` — 编译通过
2. Windows: Settings → Privacy → Microphone → 关闭 → 启动应用 → 引导页显示麦克风权限卡片 → "打开设置"按钮可用 → 授权后回到应用 → 自动重检通过
3. 关闭麦克风后按 RAlt → ErrorPanel 显示"打开麦克风设置"按钮（而非"前往设置"）
4. 错误中 user_action = "CheckMicrophone"（非 "CheckApiKey"）

### 可观测性
1. `SENTRY_DSN=xxx npm run tauri dev` → 制造 panic → Sentry 仪表板出现事件
2. 正常录音 → 日志含：device selected（名称+采样率）、Audio encoded（ms+bytes）、LLM TTFB（ms）、LLM total（ms）、Text injected（chars）
3. SLS 控制台 → 查询 `event_type = "session_completed"` 有数据
4. `npx tsc --noEmit` — TypeScript 类型检查通过
5. `npm test` — 前端测试通过

### MCP 工具
1. `claude mcp list` 显示 sentry + sls 为 connected
2. 在 Claude Code 中调用 `sentry_list_issues` 返回数据
3. 在 Claude Code 中调用 `sls_query` 返回结构化结果

### CI
1. 推送 tag → release workflow 成功
2. Sentry release 页面显示对应版本 + source maps + debug symbols

---

## 文档同步步骤（实施时执行）

1. 将本计划持久化到 `docs/plans/2026-03-02-permissions-observability.md`
2. 更新 `CLAUDE.md`：
   - 技术栈表增加 Sentry + SLS
   - 环境变量列表增加 `SENTRY_DSN`、`SLS_*`
   - Tauri 命令清单增加 `report_telemetry_event`
   - MCP 服务器章节增加 sentry + sls
3. 更新 MEMORY.md：架构决策（Sentry SaaS → 自托管迁移路径、SLS Web Tracking、telemetry trait）

---

## 前置条件（需用户操作）

1. **Sentry SaaS 账号**：访问 sentry.io 注册免费 Developer Plan（5K errors/月），获取 DSN
2. **阿里云 SLS**：创建 Project + Logstore，开启 Web Tracking 功能（免费额度 500MB/月）
3. **GitHub Secrets**：配置 SENTRY_ORG/PROJECT/AUTH_TOKEN（产品上线前增加 SENTRY_URL）
4. **环境变量**：本地开发需设置 `SENTRY_DSN`、`SLS_ENDPOINT`/`SLS_PROJECT`/`SLS_LOGSTORE`
