# 听语轩（TingYuXuan）项目文档

> 文档驱动开发（DDD）：文档是功能的规格说明，代码实现文档描述的行为。
> 唯一真值（SSOT）：每类信息只有一个权威来源，不重复记录。

## 文档导航

### 架构
- [系统架构总览](architecture/overview.md) — 分层架构、技术栈、核心组件
- [管线数据流](architecture/data-flow.md) — 事件流、离线队列、窗口管理
- [架构决策记录 (ADR)](architecture/adr/) — 所有重大架构决策的背景与理由

### 功能模块规格
- [录音与编码](modules/audio.md) — AudioRecorder + AudioBuffer + WAV 编码器 + AudioCache
- [多模态语言模型 (LLM)](modules/llm.md) — MultimodalProvider + 音频+上下文一步处理 + 提示词系统
- [管线编排](modules/pipeline.md) — Pipeline 单步多模态处理 + 重试 + 事件广播
- [文本注入](modules/text-injection.md) — X11/Wayland 文本注入 + 上下文检测
- [配置管理](modules/config.md) — AppConfig 序列化/持久化
- [历史记录](modules/history.md) — SQLite 转写记录 CRUD
- [安全模型](modules/security.md) — CSP、API Key 存储、输入验证

### 用户指南
- [安装指南](guides/installation.md)
- [使用指南](guides/usage.md)
- [配置指南](guides/configuration.md)
- [故障排查](guides/troubleshooting.md)
- [CI/Release 构建踩坑记录](guides/ci-release-notes.md) — AGP 9.0 迁移、Tauri target 路径、OOM 等

### 开发计划
- [Phase 1: MVP 核心骨架](plan/phase-1-mvp.md)
- [Phase 2: 端到端集成](plan/phase-2-integration.md)
- [Phase 3: 增强体验](plan/phase-3-enhanced.md)
- [Phase 4: 生产加固](plan/phase-4-production.md)
- [全链路可观测性增强](plan/observability-enhancement.md)
- [Phase 6: 多模态一步管线重构](plan/phase-6-multimodal-pipeline.md)
- [Phase 7: Windows 语音 MVP 修复与重构（当前执行 SSOT）](plan/phase-7-voice-mvp-remediation.md)
- [Qwen3-ASR-Flash 传输与压缩决策（2026-03-03）](plan/2026-03-03-qwen3-asr-flash-transport-decision.md)
- [MVP 整体验收报告（2026-03-03）](plan/2026-03-03-mvp-acceptance-report.md)

## 文档约定

- 架构决策 → ADR 文件（`docs/architecture/adr/`）
- 模块规格 → 模块文档（`docs/modules/`）
- 开发计划 → 计划文档（`docs/plan/`）
- 用户文档 → 指南文档（`docs/guides/`）
- 不在多处重复同一信息，而是互相引用
- 当前执行计划唯一真值：`docs/plan/phase-7-voice-mvp-remediation.md`
- 若历史 Phase 文档与当前执行计划冲突，以 Phase 7 为准（历史文档仅用于追溯）
