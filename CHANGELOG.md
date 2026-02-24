# Changelog

本文件记录 听语轩（TingYuXuan）的所有重要变更。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

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
- 全局快捷键：Ctrl+Shift+D/T/A, Esc

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
