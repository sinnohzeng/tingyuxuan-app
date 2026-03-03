# 系统架构总览

## 产品定位

听语轩（TingYuXuan）是一款 AI 驱动的智能语音输入工具，核心管线：**语音录制 → MP3 编码（失败回退 WAV）→ 多模态 LLM 一步识别+润色 → 系统级文本注入**。通过多模态大语言模型（如 Qwen3-Omni Flash、GPT-4o Audio）直接处理音频输入，单次 API 调用完成语音识别与文本处理。用户仅需配置 LLM API。

## 分层架构

```
┌─────────────────────────────────────────────────────┐
│                   React Frontend                     │
│  FloatingBar · SettingsPanel · ResultPanel · Store   │
│                   (src/)                             │
├─────────────────────────────────────────────────────┤
│                  Tauri Backend                        │
│  Commands · State · EventBridge · Tray · Shortcuts   │
│                 (src-tauri/src/)                      │
├─────────────────────────────────────────────────────┤
│               Rust Core Library                      │
│  Audio · LLM (多模态) · Pipeline · Config · History   │
│          (crates/tingyuxuan-core/src/)               │
└─────────────────────────────────────────────────────┘
```

### Rust Core (`crates/tingyuxuan-core/`)

平台无关的核心引擎，不依赖 Tauri。包含：
- **audio** — 录音器（cpal）+ AudioBuffer PCM 累积 + MP3 编码（WAV 回退）
- **llm** — LLMProvider trait + MultimodalProvider（音频+上下文 → 一步处理）+ 提示词系统
- **pipeline** — Pipeline 单步编排（音频编码 → 多模态 LLM）、事件总线
- **config** — 配置序列化（JSON）+ XDG 目录
- **history** — SQLite 转写记录管理
- **context** — InputContext 丰富上下文（应用名/窗口标题/选中文本等）
- **error** — 统一错误类型 + StructuredError

### Tauri Backend (`src-tauri/src/`)

桌面应用外壳，负责：
- **commands.rs** — Tauri 命令（录音控制、配置 CRUD、历史查询、API Key 管理、权限检查）
- **state.rs** — 独立 Managed State（见 [ADR-0002](adr/0002-split-managed-state.md)，见下方表格）
- **lib.rs** — 应用初始化、事件桥接、快捷键注册、网络监控
- **recorder_actor.rs** — 录音器 Actor（见 [ADR-0003](adr/0003-recorder-actor-pattern.md)）
- **platform/** — 平台抽象层：TextInjector + ContextDetector + FnKeyMonitor（见 [ADR-0006](adr/0006-platform-abstraction-layer.md)）
- **tray.rs** — 系统托盘菜单（快捷键标签平台自适应）

### React Frontend (`src/`)

采用 **feature-based 目录结构** + **Fluent UI 2** 组件库（详见 [前端架构](frontend.md)）。

- **features/** — 按功能模块划分：dashboard（首页）、history（历史）、dictionary（词典）、settings（设置）、onboarding（引导）、recording（录音）
- **shared/stores/** — Zustand store 分工：appStore（录音状态）、uiStore（Toast/设置面板）、statsStore（统计缓存）
- **shared/components/** — MainLayout（路由容器）、ToastHost（通知桥接）
- **shared/hooks/** — useTauriEvent（Tauri 事件监听 + mounted 守卫）
- **shared/lib/** — 类型定义、logger 工厂、主题切换

## 独立 Managed State

> 详见 [ADR-0002: 分离 Managed State 架构](adr/0002-split-managed-state.md)

| State | 类型 | 职责 |
|-------|------|------|
| ConfigState | `Arc<RwLock<AppConfig>>` | 配置（读多写少） |
| HistoryState | `Arc<Mutex<HistoryManager>>` | 历史记录 SQLite |
| PipelineState | `Arc<RwLock<Option<Arc<Pipeline>>>>` | 管线实例（可重建） |
| EventBus | `broadcast::Sender<PipelineEvent>` | 事件广播 |
| SessionState | `Arc<Mutex<Option<ActiveSession>>>` | 当前录音会话 |
| RecorderState | `RecorderHandle` | 录音器 Actor 句柄 |
| NetworkState | `Arc<AtomicBool>` | 网络连接状态 |
| InjectorState | `PlatformInjector` | 文本注入器（编译时类型别名） |
| DetectorState | `PlatformDetector` | 上下文检测器（编译时类型别名） |
| FnKeyMonitorState | `Option<FnKeyMonitor>` | Fn 键监听器（仅 macOS） |

## 技术栈

| 层 | 技术 | 版本 |
|----|------|------|
| Desktop Framework | Tauri | 2.10 |
| Backend Language | Rust | 2024 edition |
| Frontend Framework | React | 19.x |
| State Management | Zustand | 5.x |
| CSS | Tailwind CSS | 4.x |
| Audio | cpal | 0.17 |
| HTTP | reqwest | 0.13 |
| Database | rusqlite (bundled) | 0.38 |
| Async Runtime | tokio | 1.x |
| macOS Native | core-graphics + core-foundation + arboard | 0.24 / 0.10 / 3 |
| Windows Native | windows crate | 0.62 |
| Testing (Rust) | built-in + wiremock + tempfile | — |
| Testing (Frontend) | vitest + jsdom | 4.x |

## 处理模式

| 模式 | Linux/Windows | macOS | 管线行为 |
|------|---------------|-------|---------|
| Dictate（听写） | RAlt | Fn | 多模态 LLM 润色 → 自动注入 |
| Translate（翻译） | Shift+RAlt | ⌥T | 多模态 LLM 翻译 → 自动注入 |
| Edit（编辑） | 选中文本后 RAlt | 选中文本后 Fn | 多模态 LLM 编辑选中文本 → 自动注入 |
| AI Assistant | Alt+Space | ⌃Space | 多模态 LLM 自由回答 → 结果面板（不自动注入） |
