# 听语轩（TingYuXuan）项目文档

> 文档驱动开发（DDD）：文档是功能的规格说明，代码实现文档描述的行为。
> 唯一真值（SSOT）：每类信息只有一个权威来源，不重复记录。

## 文档导航

### 用户文档（安装 · 使用 · 配置）

- [安装指南](guides/installation.md) — 下载、安装、系统依赖
- [使用指南](guides/usage.md) — 快捷键、录音流程、四种模式
- [配置指南](guides/configuration.md) — API Key、语言、快捷键自定义
- [故障排查](guides/troubleshooting.md) — 常见问题与解决方案

### 开发者文档（架构 · 模块 · 贡献）

**架构**
- [系统架构总览](architecture/overview.md) — 分层架构、技术栈、核心组件
- [管线数据流](architecture/data-flow.md) — 事件流、离线队列、窗口管理
- [前端架构](architecture/frontend.md) — feature-based 目录、Zustand stores、路由
- [UI 设计规格](architecture/ui-design.md) — Fluent UI 2 组件系统、主题、无障碍
- [架构决策记录 (ADR)](architecture/adr/) — 所有重大架构决策的背景与理由

**模块规格**
- [录音与编码](modules/audio.md) — AudioRecorder + AudioBuffer + WAV/MP3 编码器
- [多模态语言模型 (LLM)](modules/llm.md) — MultimodalProvider + 音频+上下文一步处理 + 提示词系统
- [管线编排](modules/pipeline.md) — Pipeline 单步多模态处理 + 重试 + 事件广播
- [文本注入](modules/text-injection.md) — X11/Wayland/Windows 文本注入 + 上下文检测
- [配置管理](modules/config.md) — AppConfig 序列化/持久化
- [历史记录](modules/history.md) — SQLite 转写记录 CRUD
- [安全模型](modules/security.md) — CSP、API Key 存储、输入验证
- [JNI 桥接](modules/jni-bridge.md) — Android Rust JNI 接口

**贡献**
- [贡献指南](../CONTRIBUTING.md) — 开发环境搭建、代码规范、提交流程
- [CI/Release 构建踩坑记录](guides/ci-release-notes.md) — AGP 9.0 迁移、Tauri target 路径、OOM 等

### 内部文档（产品 · 计划 · 治理）

**产品**
- [产品需求文档（PRD）](prd.md) — 产品定位、功能需求、交互设计、成功指标
- [竞品分析](competitive-analysis.md) — Typeless 调研、竞品功能对比矩阵

**开发计划**

Phase 计划（`phase-N-name.md`）：里程碑级别的阶段规划。

- [Phase 1: MVP 核心骨架](plan/phase-1-mvp.md)
- [Phase 2: 端到端集成](plan/phase-2-integration.md)
- [Phase 3: 增强体验](plan/phase-3-enhanced.md)
- [Phase 4: 生产加固](plan/phase-4-production.md)
- [Phase 5: UI 大改造](plan/phase-5-ui-overhaul.md)
- [Phase 6: 多模态一步管线重构](plan/phase-6-multimodal-pipeline.md)
- [Phase 7: Windows 语音 MVP 修复与重构（当前执行 SSOT）](plan/phase-7-voice-mvp-remediation.md)

Sprint 实施计划（`YYYY-MM-DD-topic.md`）：具体任务级别的实施方案。

- [全链路可观测性增强方案](plan/observability-enhancement.md)
- [Sprint 8: 权限系统 + 可观测性实施](plan/2026-03-02-permissions-observability.md)
- [托盘菜单重构 + 麦克风设备选择](plan/2026-03-03-tray-menu-redesign.md)
- [Qwen3-ASR-Flash 传输与压缩决策](plan/2026-03-03-qwen3-asr-flash-transport-decision.md)
- [MVP 整体验收报告](plan/2026-03-03-mvp-acceptance-report.md)
- [文档系统性治理计划](plan/2026-03-03-documentation-governance.md)
- [文档自动化工具链](plan/2026-03-03-documentation-automation.md)

## 文档约定

- 架构决策 → ADR 文件（`docs/architecture/adr/`）
- 模块规格 → 模块文档（`docs/modules/`）
- 开发计划 → 计划文档（`docs/plan/`），禁止使用 `docs/plans/`
- 用户文档 → 指南文档（`docs/guides/`）
- 不在多处重复同一信息，而是互相引用
- 当前执行计划唯一真值：`docs/plan/phase-7-voice-mvp-remediation.md`
- 若历史 Phase 文档与当前执行计划冲突，以 Phase 7 为准（历史文档仅用于追溯）

## 文档治理规则

### 权威来源定义

| 信息类型 | 唯一权威文档 |
|----------|-------------|
| AI 助手上下文 | `CLAUDE.md` |
| 产品需求 | `docs/prd.md` |
| 系统架构 | `docs/architecture/overview.md` |
| 模块规格 | `docs/modules/*.md` |
| 版本历史 | `CHANGELOG.md` |
| 当前执行计划 | `CLAUDE.md` 中声明的 SSOT 文件 |
| 竞品信息 | `docs/competitive-analysis.md` |

### Sprint 完成检查清单

每次 Sprint / 版本发布前，必须检查并更新：

1. `CHANGELOG.md` — 添加新版本条目
2. `CLAUDE.md` — 命令清单、测试计数、技术栈变更
3. `docs/README.md` — 导航索引覆盖新增文档
4. 相关 `docs/modules/*.md` — 接口变更同步
5. `README.md` — 面向用户的重大变更
6. `docs/prd.md` 第七节 — 实现状态追踪更新
7. 运行 `npm run lint:docs` 确认文档格式无违规
