# 听语轩 Phase 4：生产加固 & v0.1.0 发布准备（修订版）

## Context

阶段三完成后，听语轩已具备完整的端到端功能链路。然而从**生产发布**角度审视，存在质量缺口、安全隐患和文档空白。阶段四目标：**修复生产级质量缺口，建立文档驱动的工程规范，达到可交付 v0.1.0 标准**。

当前基线：91 Rust 测试 + 13 前端测试 | 前端构建零错误 | 3 次 Git commit

---

## 架构师评审：原方案改进点

对原方案从资深架构师角度的评审与修正：

| # | 原方案问题 | 改进 |
|---|-----------|------|
| 1 | **缺少文档基础设施** — v0.1.0 即将发布，但项目无任何结构化文档（无架构文档、无 ADR、无用户指南） | 新增 Step 0：文档基础设施，作为 DDD/SSOT 原则的落地载体 |
| 2 | **跨平台 text injector trait 重构过早** — v0.1.0 仅发布 Linux，重构为 trait 增加复杂度但无即时收益 | 移除原 Step 6，延至 Phase 5（当真正需要 macOS/Windows 时再做） |
| 3 | **缺少用户文档** — 发布软件无安装指南、使用指南、故障排查文档 | 纳入 Step 7 发布步骤 |
| 4 | **过去的架构决策未记录** — 6+2 分离 state、Actor 模式、事件桥接等关键决策仅存在于开发者脑中 | Step 0 中补写 ADR 记录 |
| 5 | **CI/CD 仅有构建，缺少质量门禁** — 无 clippy deny、无覆盖率报告、无 changelog 自动化 | Step 1 增强 CI 质量门禁 |
| 6 | **配置迁移实现过于简化** — v0 → v1 迁移本身是 trivial（serde default 已处理），但框架的价值在于后续版本 | 保留但定位为"建立基线框架"而非解决当前问题 |

---

## 工程原则：DDD + SSOT

本阶段起，项目采用两项核心工程原则：

### 文档驱动开发（DDD）
> "From the perspective of a user, if a feature is not documented, then it doesn't exist." — [Documentation-Driven Development](https://gist.github.com/zsup/9434452)

- 每个功能模块有对应文档，文档是功能的规格说明
- 文档先于或与代码同步编写，代码实现文档描述的行为
- 文档与代码版本同步

### 唯一真值（SSOT）
> "One place — and only one place — where each type of information lives." — [Atlassian SSOT Guide](https://www.atlassian.com/work-management/knowledge-sharing/documentation/building-a-single-source-of-truth-ssot-for-your-team)

- 每类信息只有一个权威来源
- 架构决策 → ADR 文件（不散落在 PR、聊天记录中）
- 模块规格 → `docs/modules/` 下对应文档
- 开发计划 → `docs/plan/` 下版本化计划文件
- 不重复记录同一信息，而是互相引用

### 项目文档结构

```
docs/
├── README.md                           # 文档导航索引
├── architecture/
│   ├── overview.md                     # 系统架构总览
│   ├── data-flow.md                    # 管线数据流
│   └── adr/                            # 架构决策记录
│       ├── 0001-tauri-framework.md
│       ├── 0002-split-managed-state.md
│       ├── 0003-recorder-actor-pattern.md
│       ├── 0004-event-bridge-push-model.md
│       ├── 0005-keyring-api-key-storage.md
│       └── template.md
├── plan/
│   ├── phase-1-mvp.md
│   ├── phase-2-integration.md
│   ├── phase-3-enhanced.md
│   └── phase-4-production.md           # 本阶段计划
├── modules/
│   ├── audio.md                        # 录音 & 缓存
│   ├── stt.md                          # STT 语音识别
│   ├── llm.md                          # LLM 润色 & 提示词
│   ├── pipeline.md                     # 管线编排 & 离线队列
│   ├── text-injection.md               # 文本注入
│   ├── config.md                       # 配置管理
│   ├── history.md                      # 历史记录
│   └── security.md                     # 安全模型
└── guides/
    ├── installation.md                 # 用户安装指南
    ├── usage.md                        # 使用指南
    ├── configuration.md                # 配置指南
    └── troubleshooting.md              # 故障排查
```

### ADR 模板

```markdown
# ADR-NNNN: [标题]

**状态**: Proposed | Accepted | Deprecated | Superseded by ADR-XXXX
**日期**: YYYY-MM-DD

## 背景
是什么问题促使了这个决策？

## 决策
我们选择了什么方案？

## 后果
这个决策带来了什么正面和负面影响？

## 备选方案
还考虑了哪些方案？为什么没有选择？
```

---

## 依赖关系图

```
Step 0: 文档基础设施（DDD/SSOT 落地）
    ├── Step 1: CI/CD 基础设施
    │       ├── Step 2: STT/LLM Provider wiremock 集成测试
    │       ├── Step 3: 安全加固（CSP + 输入验证）
    │       └── Step 4: 持久化离线队列（SQLite）
    │               └── Step 5: 配置版本管理
    ├── Step 6: 前端 Error Boundary + 组件测试
    └── Step 7: 发布准备（打包 + 用户文档 + v0.1.0）
```

Step 0 首先完成（文档先行）。Step 2/3/4 互相独立。Step 6 独立。Step 7 依赖全部完成。

---

## Step 0: 文档基础设施 — DDD/SSOT 落地

**目标**：建立项目文档结构，追溯记录所有架构决策，为每个功能模块编写规格文档。这是 DDD 原则的落地——文档成为项目的唯一真值来源。

### 0.1 创建文档目录结构

按上述 `docs/` 结构创建所有目录和文件。

### 0.2 系统架构文档

**新建文件**：`docs/architecture/overview.md`

内容要点：
- 系统分层：Rust Core (`crates/tingyuxuan-core/`) → Tauri Backend (`src-tauri/`) → React Frontend (`src/`)
- 8 个独立 Managed State 架构图
- 核心管线流程：录音 → STT → LLM → 文本注入
- 技术栈选型总结

**新建文件**：`docs/architecture/data-flow.md`

内容要点：
- 管线事件流（`broadcast::Sender<PipelineEvent>` → `app.emit` → 前端 `listen`）
- 离线队列数据流
- 窗口可见性管理逻辑

### 0.3 架构决策记录（ADR）

基于已做出的架构决策，追溯编写 5 个 ADR：

1. **ADR-0001: 选择 Tauri 2.0 框架** — Tauri vs Electron vs 原生，选择 Tauri 的安全性、包体积优势
2. **ADR-0002: 分离 Managed State 架构** — 8 个独立 State 取代单一 `AppState` Mutex，减少锁竞争
3. **ADR-0003: 录音器 Actor 模式** — 专用 OS 线程 + mpsc channel，避免音频回调阻塞 tokio runtime
4. **ADR-0004: 事件桥接 Push 模式** — broadcast → app.emit 取代前端轮询，实现实时 UI 更新
5. **ADR-0005: OS Keyring API Key 存储** — keyring crate + 明文降级方案

### 0.4 功能模块文档

为 `docs/modules/` 下 8 个模块各编写规格文档。每个文档包含：
- 模块职责（1-2 句话）
- 关键类型/trait 定义（从代码摘录）
- 公开 API 列表
- 错误处理策略
- 测试覆盖说明
- 已知限制

具体内容基于对应源代码：
- `audio.md` ← `crates/tingyuxuan-core/src/audio/recorder.rs` + `cache.rs`
- `stt.md` ← `crates/tingyuxuan-core/src/stt/` (trait + 3 个 provider)
- `llm.md` ← `crates/tingyuxuan-core/src/llm/` (trait + prompts)
- `pipeline.md` ← `crates/tingyuxuan-core/src/pipeline/` (orchestrator + queue + events + retry + recovery)
- `text-injection.md` ← `src-tauri/src/text_injector.rs` + `context.rs`
- `config.md` ← `crates/tingyuxuan-core/src/config.rs`
- `history.md` ← `crates/tingyuxuan-core/src/history.rs`
- `security.md` ← CSP 策略、keyring 存储、输入验证

### 0.5 开发计划归档

将阶段一至四的开发计划整理到 `docs/plan/`：
- `phase-1-mvp.md` — 简要回顾（已完成）
- `phase-2-integration.md` — 简要回顾（已完成）
- `phase-3-enhanced.md` — 基于已执行的计划整理（已完成）
- `phase-4-production.md` — 本计划文档

### 0.6 Claude 长期记忆

**新建/更新文件**：`/home/ecs-user/.claude/projects/-data-workspace-tingyuxuan-app/memory/MEMORY.md`

记录项目核心上下文、架构决策、工程原则，供后续会话复用。

**验证**：`docs/` 目录结构完整，所有文档可读且内容准确。

---

## Step 1: CI/CD 基础设施

**目标**：建立自动化质量门禁。

**新建文件**：`.github/workflows/ci.yml`

三个并行 Job：

1. **rust-check**：
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test -p tingyuxuan-core`（`TINGYUXUAN_MOCK_AUDIO=1`）
   - 系统依赖：`libasound2-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`

2. **frontend-check**：
   - `npx tsc --noEmit`
   - `npm test`

3. **build**（依赖前两个通过）：
   - `npm run build && cargo build --release -p tingyuxuan-app`

**验证**：推送后三个 Job 全部绿色。

---

## Step 2: STT/LLM Provider wiremock 集成测试

**目标**：为三个 Provider 添加 HTTP 级集成测试，覆盖 5 种场景（成功/401/429/500/畸形响应）。Provider 是唯一外部 API 交互模块，是**最高 ROI** 的测试投入。

**修改文件**：
- `crates/tingyuxuan-core/src/stt/whisper.rs` — 添加 `#[cfg(test)] mod tests`
- `crates/tingyuxuan-core/src/stt/dashscope_asr.rs` — 添加 `#[cfg(test)] mod tests`
- `crates/tingyuxuan-core/src/llm/openai_compat.rs` — 添加 `#[cfg(test)] mod tests`

**wiremock 已在 dev-dependencies** 中（`Cargo.toml` line 36）。

### 每个 Provider 6 个测试

辅助函数：创建最小有效 WAV（hound 写入 160 个零采样到 NamedTempFile）。

**WhisperProvider**：Mock `POST /audio/transcriptions`
- 200 → text 提取正确
- 401 → `STTError::AuthFailed`
- 429 → `STTError::RateLimited`
- 500 → `STTError::ServerError`
- 200 + 畸形 body → 解析错误
- GET `/models` 200 → `test_connection()` true

**DashScopeASRProvider**：Mock `POST /compatible-mode/v1/chat/completions`
- 同上 5 种场景 + test_connection

**OpenAICompatProvider**：Mock `POST /chat/completions`
- 200 → `processed_text` 正确，`tokens_used` 解析
- 同上 4 种错误场景 + test_connection

**新增测试**：~18 个。Rust 总计 → ~109。

**文档同步**：更新 `docs/modules/stt.md` 和 `docs/modules/llm.md` 的测试覆盖说明。

---

## Step 3: 安全加固

**目标**：关闭 CSP 漏洞，添加输入验证，防止注入攻击。

### 3.1 CSP 策略

**修改文件**：`src-tauri/tauri.conf.json` line 40

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
```

### 3.2 输入验证

**修改文件**：`src-tauri/src/commands.rs`

```rust
const MAX_INJECT_TEXT_LEN: usize = 50_000;
const MAX_API_KEY_LEN: usize = 512;
const MAX_SEARCH_QUERY_LEN: usize = 500;
const MAX_DICT_WORD_LEN: usize = 100;
```

在每个命令入口处校验：mode 合法性、长度限制、null 字节检查。

### 3.3 文本注入防护

**修改文件**：`src-tauri/src/text_injector.rs`

添加 `sanitize_for_typing()` 过滤控制字符（保留 `\n`、`\t`），应用于非 clipboard 路径。

**新增测试**：5 个。

**文档同步**：更新 `docs/modules/security.md`。

---

## Step 4: 持久化离线队列

**目标**：将内存 `OfflineQueue` 替换为 SQLite 持久化，排队录音在崩溃后不丢失。

**修改文件**：
- `crates/tingyuxuan-core/src/pipeline/queue.rs` — 重写为 `PersistentQueue`
- `src-tauri/src/state.rs` line 36 — 更新 `QueueState` 类型
- `src-tauri/src/lib.rs` line 82 — 更新初始化

### 核心设计

- SQLite 表 `queue`：`session_id TEXT PK, audio_path, mode, target_language, selected_text, app_context, created_at`
- `ProcessingMode` 序列化为字符串
- `enqueue()` 接受 `&self` + `&QueuedRecording`（与现有 API 兼容，仅从 `&mut self` 改为 `&self`）
- `drain()` 在事务中 SELECT + DELETE
- `new_in_memory()` 用于测试和降级
- 初始化失败时自动降级到内存模式

### API 兼容性

`enqueue`/`drain`/`len`/`is_empty` 签名保持一致，`commands.rs` 和 `lib.rs` 中的调用代码**仅需修改** `enqueue` 从 `&mut self` 到 `&self`（SQLite 内部 mutex）。

**新增测试**：8 个（enqueue/drain FIFO、持久化、空 drain、重复 session_id、降级）。

**文档同步**：更新 `docs/modules/pipeline.md`，新增 ADR-0006: 持久化离线队列。

---

## Step 5: 配置版本管理

**目标**：建立版本化配置迁移框架，为后续升级铺路。

**修改文件**：`crates/tingyuxuan-core/src/config.rs`

### 实现

- 新增 `#[serde(default)] config_version: u32` 字段
- 新增 `load_with_migration()` 方法：检测版本 → 逐版本迁移 → 备份旧配置 → 保存
- `migrate_v0_to_v1()`：设置版本号（字段本身由 `serde(default)` 处理）
- 更新 `state.rs` 使用 `load_with_migration()`

**定位**：这是一个**基线框架**。v0 → v1 迁移本身是 trivial，但框架的价值在于后续版本可以安全地修改配置结构。

**新增测试**：5 个（旧配置迁移、备份生成、默认版本号、序列化含版本、往返一致性）。

**文档同步**：更新 `docs/modules/config.md`。

---

## Step 6: 前端 Error Boundary + 组件测试

**目标**：防止白屏崩溃，为关键 UI 路径添加测试。

### 6.1 Error Boundary

**新建文件**：`src/components/ErrorBoundary.tsx`

- `getDerivedStateFromError` 捕获渲染错误
- 显示中文错误信息 + "重试"按钮
- `componentDidCatch` 输出到 console

**修改文件**：`src/App.tsx` — 包裹 `<ErrorBoundary>`

### 6.2 安装测试依赖

`npm install -D @testing-library/react @testing-library/jest-dom`

### 6.3 组件测试

**新建文件**：
- `src/components/FloatingBar.test.tsx` — 5 个状态测试（idle/recording/processing/error/done+ai）
- `src/components/ResultPanel.test.tsx` — 3 个测试（渲染/复制/关闭）

Mock Tauri API（`@tauri-apps/api/event`、`@tauri-apps/api/core`、`@tauri-apps/api/window`）。

**新增测试**：~10 个前端测试。总前端 → ~23。

---

## Step 7: 发布准备 — 打包 + 用户文档 + v0.1.0

**目标**：完成所有发布物料，打标签。

### 7.1 Tauri 打包配置

**修改文件**：`src-tauri/tauri.conf.json`

```json
"bundle": {
    "active": true,
    "targets": ["deb", "appimage"],
    "linux": {
        "deb": { "depends": ["libasound2", "libwebkit2gtk-4.1-0"], "section": "utils" }
    },
    "shortDescription": "AI-powered voice input tool",
    "longDescription": "TingYuXuan converts speech to polished text using configurable STT and LLM APIs."
}
```

### 7.2 Release Workflow

**新建文件**：`.github/workflows/release.yml`

触发：`push tags: ['v*']`。构建 `.deb` + `.AppImage`，上传 artifacts。

### 7.3 用户文档

**编写文件**：
- `docs/guides/installation.md` — Linux 安装指南（.deb / .AppImage / 源码编译）
- `docs/guides/usage.md` — 快捷键、三种模式、AI 助手、设置说明
- `docs/guides/configuration.md` — Provider 配置指南（DashScope / OpenAI / 自定义）
- `docs/guides/troubleshooting.md` — 常见问题（xdotool 缺失、Wayland 限制、keyring 不可用等）

### 7.4 CHANGELOG

**新建文件**：`CHANGELOG.md`

v0.1.0 全部功能清单。

### 7.5 项目 README 更新

更新根目录 `README.md`：项目简介、特性列表、安装方式、文档链接。

**验证**：`git tag v0.1.0` 触发 release workflow，产出 `.deb` + `.AppImage`。

---

## 测试目标汇总

| 步骤 | 新增 Rust | 新增前端 | 累计 |
|------|----------|---------|------|
| Step 0 (文档) | 0 | 0 | 91 + 13 |
| Step 1 (CI) | 0 | 0 | 91 + 13 |
| Step 2 (Provider) | ~18 | 0 | ~109 + 13 |
| Step 3 (安全) | ~5 | 0 | ~114 + 13 |
| Step 4 (队列) | ~8 | 0 | ~122 + 13 |
| Step 5 (配置) | ~5 | 0 | ~127 + 13 |
| Step 6 (前端) | 0 | ~10 | ~127 + 23 |
| Step 7 (发布) | 0 | 0 | **~127 + 23 = 150** |

## 验证矩阵

| 步骤 | `cargo test` | `npm run build` | `npm test` |
|------|-------------|-----------------|------------|
| Step 0 | 91 pass | pass | 13 pass |
| Step 1 | 91 pass | pass | 13 pass |
| Step 2 | ~109 pass | pass | 13 pass |
| Step 3 | ~114 pass | pass | 13 pass |
| Step 4 | ~122 pass | pass | 13 pass |
| Step 5 | ~127 pass | pass | 13 pass |
| Step 6 | ~127 pass | pass | ~23 pass |
| Step 7 | ~127 pass | pass | ~23 pass |

---

## 实施后续动作

计划批准后，实施时还将同步：
1. 创建 Claude 长期记忆文件（`MEMORY.md`），记录项目上下文、架构、工程原则
2. 每完成一个 Step 执行 Git commit
3. 每个 Step 完成后更新对应的模块文档（DDD 原则）
