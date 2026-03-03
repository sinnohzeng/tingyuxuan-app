# v0.10.3 技术债修复与文档同步执行计划（DDD/SSOT）

## 元信息

- 计划日期：2026-03-03
- 目标版本：`v0.10.3`
- 执行范围：全栈一次整改（Desktop + Frontend + Android + 文档 + CI）
- 红线统计范围：仅生产代码（`src`、`src-tauri/src`、`crates/*/src`、`android/app/src/main`）
- 质量门禁策略：立即硬门禁（CI 阻断）
- 当前状态：待发布（代码与文档整改完成，等待提交/tag/release）

## 背景

当前仓库存在三类技术债：

1. 代码红线落地不完整：单函数行数、分支数、嵌套层级存在系统性超标。
2. 架构语义漂移：Android 与部分文档仍保留 STT+LLM 双阶段叙述，与当前单步多模态实现不一致。
3. 文档与工程治理未闭环：缺少可执行的红线门禁脚本与 CI 强制检查，易导致债务回流。

## 目标

1. 建立并启用红线质量门禁，确保新增代码不突破阈值。
2. 清理核心热点文件的结构性技术债，降低复杂度并保持行为兼容。
3. 消除 STT 旧语义，统一到单步多模态架构。
4. 按 DDD/SSOT 同步更新所有权威文档，避免多真值源。
5. 完成 `v0.10.3` 版本发布闭环（提交、tag、release）。

## 非目标

1. 不引入破坏性公共接口变更（Tauri command / JNI 导出签名保持兼容）。
2. 不在本次新增长期第二记忆载体（禁止新增 MEMORY 文件）。
3. 不扩展与本次技术债无关的新功能。

## 执行阶段

### 阶段 A：SSOT 固化（P0）

1. 新建本计划文件并纳入 `docs/README.md` 导航。
2. 将 `CLAUDE.md` 的当前执行计划切换至本文件。
3. 在文档中固定红线统计范围和门禁策略。

### 阶段 B：质量门禁落地（P0）

1. 新增红线检查脚本（文件行数、函数行数、嵌套层级、分支数量）。
2. 新增 `npm run quality:redline` 命令。
3. 在 CI 中引入红线检查并阻断合并。

### 阶段 C：Rust 热点重构（P0）

1. `src-tauri/src/lib.rs`：拆分启动流程、事件桥接、快捷键处理函数。
2. `src-tauri/src/commands.rs`：拆分录音会话启动/停止流程，降低单函数复杂度。
3. `crates/tingyuxuan-core/src/pipeline/orchestrator.rs`：拆分 `process_audio` 流程阶段。
4. `crates/tingyuxuan-core/src/audio/recorder.rs`：拆分采样处理与编码前置逻辑。

### 阶段 D：平台层与前端重构（P0/P1）

1. Windows 平台层文件拆分为子模块，保留现有行为与接口。
2. 前端重点拆分：
   - `src/features/recording/FloatingBar.tsx`
   - `src/shared/components/MainLayout.tsx`
   - `src/shared/lib/markdown.ts`
   - 超长 hooks（`useConfig`、`useApiKey`、`useHistory`、`useDictionary`）

### 阶段 E：Android 语义对齐（P0/P1）

1. 移除 STT 必填配置语义，收敛为仅 LLM 必填。
2. `ConfigStore.kt`：读取兼容旧 `stt` 字段，但不再写入 `stt`。
3. `SettingsActivity.kt` / `OnboardingActivity.kt` / `TingYuXuanIMEService.kt` / `TingYuXuanKeyboard.kt`：清理 STT 旧术语并拆分长函数。

### 阶段 F：文档与版本发布（P0）

1. 同步模块文档、架构文档、ADR、README、PRD、CHANGELOG。
2. 统一版本到 `0.10.3`，补齐 `0.10.2` 记录。
3. 完成中文 commit message、tag `v0.10.3`、GitHub release 触发。

## 接口与兼容性边界

1. Tauri commands 名称与参数保持不变。
2. JNI 导出签名保持不变。
3. Android 配置 JSON 改为“仅 LLM 必填”，旧配置兼容读取迁移。
4. 新增工程接口：`quality:redline`（CI 必过）。

## 验收标准

1. 质量门禁：`npm run quality:redline` 本地与 CI 均通过。
2. 前端：`npm run lint`、`npx tsc --noEmit`、`npm test` 全通过。
3. Rust：`cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings`、`cargo test -p tingyuxuan-core --no-default-features`、`cargo test -p tingyuxuan-jni` 全通过。
4. Android：`./gradlew testDebugUnitTest` 通过；若本地环境缺少 Java，由 CI 验证并在发布说明记录。
5. 文档：路径引用有效、术语一致、SSOT 指向唯一。

## 文档同步矩阵（本次必更）

1. 架构总览：`docs/architecture/overview.md`
2. 模块规格：
   - `docs/modules/audio.md`
   - `docs/modules/pipeline.md`
   - `docs/modules/security.md`
   - `docs/modules/history.md`
   - `docs/modules/jni-bridge.md`
   - `docs/modules/config.md`
3. ADR：`docs/architecture/adr/0007-android-native-ime.md`
4. 对外文档：
   - `README.md`
   - `docs/prd.md`
   - `src-tauri/tauri.conf.json`（`longDescription`）
   - `CHANGELOG.md`
5. 长期记忆：`CLAUDE.md`（仅保留稳定约束，收敛易变数据）

## 执行检查清单

- [x] 计划文件创建并落盘到 `docs/plan/`
- [x] CI 红线门禁脚本与阻断接入完成
- [x] Rust 核心热点重构完成
- [x] Windows 平台层拆分完成
- [x] 前端热点重构完成
- [x] Android 语义对齐与热点重构完成
- [x] 文档矩阵同步完成
- [ ] 版本、提交、tag、release 完成

## 阶段验收记录（2026-03-03）

- `npm run quality:redline`：通过（0 violations）
- `npm run lint`：通过
- `npx tsc --noEmit`：通过
- `npm test`：通过（16 files / 74 tests）
- `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings`：通过
- `cargo test --manifest-path src-tauri/Cargo.toml -p tingyuxuan-core --no-default-features`：通过（120 tests）
- `cargo test --manifest-path src-tauri/Cargo.toml -p tingyuxuan-jni`：通过（13 tests）
- `./gradlew testDebugUnitTest`：本地失败（缺少 `JAVA_HOME` 与 `java` 命令），由 CI 覆盖验证
