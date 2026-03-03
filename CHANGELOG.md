# Changelog

本文件记录 听语轩（TingYuXuan）的所有重要变更。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

---

## [0.10.1] - 2026-03-03

### 变更

**语音 MVP 闭环整改**
- 运行模型固定为 `qwen3-omni-flash`，程序层统一控制，不开放用户切换
- 连接测试升级为 `test_multimodal_connection`（真实音频探测多模态能力）
- 新增模板占位文本质量闸门，拦截"请开始录音"类伪结果
- 事件/状态机落地 6 态：`idle → starting → recording → thinking → done/error/cancelled`
- 编码链路改为 MP3 优先、WAV 回退；服务器拒绝 MP3 时自动回退 WAV 重试
- Prompt 重构：反模板约束、去口语化、结构化输出、词典优先
- 录音时长治理：≤5 分钟，第 4 分钟起倒计时警告，5 分钟自动停止

---

## [0.10.0] - 2026-03-03

### 新增

**托盘菜单全面重构**
- 托盘菜单对标 Typeless：反馈/设置/麦克风选择/词典/版本/更新/退出
- 麦克风设备选择：枚举输入设备子菜单，持久化设备 ID 到 AudioConfig
- 设备 fallback：找不到指定设备时自动回退到系统默认设备
- 惰性菜单重建：每次右键托盘时重新枚举设备并重建子菜单
- ADR-0009：音频设备选择架构决策
- `tauri-plugin-opener` 用于托盘菜单 URL 打开
- 编译时元数据：`env!("CARGO_PKG_REPOSITORY")` 和 `env!("CARGO_PKG_VERSION")`

---

## [0.9.0] - 2026-03-03

### 新增

**全链路可观测性 + 全平台权限系统**
- Sentry 集成（`sentry` 0.42 + `tauri-plugin-sentry` 0.5）崩溃/错误上报
- SLS Web Tracking 匿名埋点（零认证 JSON POST，30s/50 条 buffer flush）
- `TelemetryBackend` trait：`SlsTransport`（生产）+ `NoopBackend`（未配置时）
- 前端通过 `report_telemetry_event` Tauri command 上报
- 结构化错误传递：`StructuredError` JSON，前端解析 `error_code` + `user_action`
- 全平台权限检测：`PermissionReport`（microphone/accessibility/input_monitoring）
- `probe_microphone()` 静态方法验证麦克风可用性
- MCP 工具：Sentry MCP（查错误）+ SLS MCP（查埋点）

---

## [0.8.1] - 2026-03-01

### 修复

- 修复 Windows 托盘图标不显示问题
- 修复 RAlt 录音快捷键在某些场景下无响应
- 修复托盘左键点击行为（弹出主窗口）
- 修复浮动条在某些分辨率下不可见
- cargo fmt + collapsible_if 全面格式修正
- 修复 CI/Release 构建失败（clippy + ESLint + 缺失 logo）

---

## [0.8.0] - 2026-03-01

### 新增

**Phase 5: UI 大改造**
- Feature-based 前端目录重组（6 个功能模块 + shared/）
- Fluent UI 2 组件库集成（@fluentui/react-components v9）
- 首页仪表盘：统计卡片 + 最近转录列表
- 历史记录页：分页浏览、搜索、批量删除
- 词典管理页：词汇标签网格
- 设置弹窗：API/语言/快捷键/通用/词典分区
- 引导流程：IntroSlide → SetupWizard → PermissionGuide → 进入首页
- 托盘双击弹出主窗口 + 关闭缩小到托盘（可配置）
- Toast 错误通知系统替代静默 catch
- `useTauriEvent` 自定义 hook（mounted 守卫）
- Zustand stores：appStore + uiStore + statsStore

**Phase 6: 多模态一步管线重构**
- 移除独立 STT 模块，统一为 MultimodalProvider 单步处理
- SSE 流式解析（Qwen-Omni `stream=true`）
- 配置版本 v1→v2 迁移（删除 STT 配置）

**Sprint 7: 核心可用性修复**
- 浮动条增强 + RAlt 独占录音 + 配置简化
- MiniMax MCP 服务器配置
- Pipeline TOCTOU 防护 + 重试跳过不可重试错误

### 变更

- Rust AggregateStats 聚合查询 + `get_dashboard_stats` 命令
- 测试：131 Rust + 71 vitest（test-utils.tsx 提供 renderWithProviders）
- console.error → createLogger 标准化迁移

---

## [0.7.3] - 2026-03-01

### 修复

- Windows 用 `WH_KEYBOARD_LL` 低级键盘钩子修复 RAlt 快捷键无法触发

---

## [0.7.2] - 2026-03-01

### 修复

- 修复 tracing span 异步安全问题
- 修复 NetworkMonitor 运行时上下文

---

## [0.7.1] - 2026-03-01

### 新增

- 全链路可观测性增强：Span Tree 替代散落日志行
- v0.7.0 全项目文档同步更新（DDD/SSOT）

---

## [0.7.0] - 2026-02-28

### 新增

**macOS 平台层原生重构 + 全平台 CI**
- macOS 原生平台层：CGEvent 键盘注入、NSPasteboard 剪贴板、NSWorkspace 上下文检测
- macOS Fn 键监听（CGEventTap）
- 全平台 CI：Linux + Windows + macOS clippy + 测试
- CI/Release 构建踩坑记录更新

---

## [0.6.0] - 2026-02-28

### 新增

**流式语音识别 + 核心架构重构**
- 流式 STT 管线重构 + 技术债全面梳理
- Android Tink 加密库 ProGuard 规则

---

## [0.5.0] - 2026-02-27

### 新增

**全面质量提升 + Android MVI 重构**
- Android 端 6 个致命 Bug 修复并重构为 MVI 架构
- 添加 CLAUDE.md 项目上下文文件
- 添加 CI/Release 构建踩坑指南

---

## [0.4.0] - 2026-02-25

### 变更

**快捷键重设计**
- 默认快捷键从 `Ctrl+Shift+D/T/A` 改为以右 Alt 为核心的方案
  - 听写：`RAlt`（右 Alt 单按）
  - 翻译：`Shift+RAlt`
  - AI 助手：`Alt+Space`
- 更新托盘菜单、前端设置面板、所有文档

### 新增

**Release 工作流重构**
- Fan-out / fan-in 架构：build jobs → create-release（消除并发竞态）
- 新增 Android APK 构建（cargo-ndk + Gradle assembleRelease）
- 添加 SHA256SUMS.txt 校验文件
- 所有 release 文件名添加版本号前缀
- 移除 release 构建中的重复测试（CI workflow 已覆盖）

**许可证**
- 添加 Source-Available 许可证（代码公开仅供参考和学习）

**工程**
- 生成 Gradle wrapper（8.12，兼容 AGP 9.0.1）
- 版本号统一为 0.4.0（package.json, Cargo.toml, tauri.conf.json, build.gradle.kts）
- 补打 v0.1.0 ~ v0.3.0 历史标签

---

## [0.3.0] - 2026-02-25

### 新增

**Android 支持**
- Android 原生输入法（InputMethodService）
- Rust JNI 桥接层（tingyuxuan-jni crate）+ generation-based handle table
- Material 3 Compose 键盘 UI（录音按钮、模式切换）
- Android AudioRecord 16kHz mono WAV 录音
- EncryptedSharedPreferences API Key 安全存储
- 设置界面（API Key 配置）
- commitText() 文本注入（无需剪贴板）
- 支持架构：arm64-v8a, armeabi-v7a, x86_64

**Core 改进**
- cpal/hound 设为 optional feature（Android 无需桌面音频）
- tingyuxuan-core 支持 `--no-default-features` 编译
- ADR-0007: Android 原生 IME 架构决策

---

## [0.2.0] - 2026-02-25

### 新增

**Windows 支持**
- 平台抽象层：TextInjector / ContextDetector trait + 编译时类型别名（零开销）
- Windows 文本注入：SendInput 批量提交 + 剪贴板 Ctrl+V（>200 字符自动切换）
- Windows 上下文检测：GetForegroundWindow 活动窗口 + Ctrl+C 选中文本
- Windows 安装包：MSI + NSIS 双格式（NSIS 支持中英文语言选择）
- Windows CI/CD：GitHub Actions windows-latest 构建 + 测试
- Windows Keyring：Credential Manager 原生支持

**工程改进**
- PlatformError 结构化错误（thiserror），替代 Result<_, String>
- InjectorState / DetectorState 作为 Tauri Managed State（总计 10 个）
- 剪贴板 save/write/paste/restore DRY 抽象（primitive 函数 + 组合）
- 所有 unsafe 块配 // SAFETY: 注释
- 平台操作添加 tracing info_span
- 交叉编译检查：Linux CI 运行 cargo check --target x86_64-pc-windows-msvc
- ADR-0006: 平台抽象层设计决策

---

## [0.1.0] - 2026-02-24

### 新增

**核心功能**
- 语音录音与 WAV 编码（CPAL + hound）
- STT 语音识别（支持 Whisper API、阿里云 DashScope ASR、自定义 Provider）
- LLM 智能润色（支持 OpenAI、DashScope、火山引擎、自定义 Provider）
- 四种输入模式：听写、翻译、AI 助手、编辑
- Linux 文本注入（X11: xdotool/xclip, Wayland: wtype/wl-clipboard）
- 全局快捷键：RAlt / Shift+RAlt / Alt+Space / Esc

**管线与可靠性**
- 管线编排：录音 → STT → LLM → 文本注入
- SQLite 持久化离线队列（网络恢复后自动处理）
- 网络状态监测（30 秒轮询）
- 崩溃恢复（未完成录音的自动检测与恢复）

**前端**
- React 19 悬浮栏（录音状态、音量可视化、结果展示）
- Zustand 状态管理
- Error Boundary 防白屏
- AI 助手结果面板（Markdown 渲染、复制、插入）
- 设置窗口（Provider 配置、语言、词典、连接测试）

**安全**
- OS Keyring API Key 安全存储（降级明文备选）
- CSP 严格策略（禁止外部脚本和内联脚本）
- Tauri Commands 输入验证（长度限制、null 字节检查、参数白名单）
- 文本注入控制字符过滤

**工程**
- GitHub Actions CI/CD（Rust 检查 + 前端检查 + 构建）
- 117 个 Rust 测试 + 26 个前端测试
- 配置版本管理与迁移框架
- DDD 文档驱动开发 + SSOT 唯一真值文档体系
- 5 个架构决策记录（ADR）
