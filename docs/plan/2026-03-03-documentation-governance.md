# 项目文档系统性治理计划

> 状态：✅ 已完成 | 执行日期：2026-03-03

## 背景

项目经历了从 v0.1.0 到 v0.10.1 的快速迭代（Phase 1→7），文档体系出现了以下系统性问题：

1. **目录结构分裂**：`docs/plan/` 和 `docs/plans/` 两个目录并存，违反 SSOT
2. **README.md 严重过时**：仍描述 STT+LLM 两步架构，技术栈、测试数等 6 处事实错误
3. **CHANGELOG.md 缺失 11 个版本**：仅覆盖到 v0.4.0，缺少 v0.5.0～v0.10.1
4. **CLAUDE.md 命令表不完整**：缺少 3 个命令，2 个返回值有误
5. **docs/README.md 导航缺失**：5 个已有文档未收录到导航索引
6. **PRD 严重过时**：1198 行，混合了产品需求与实现细节，架构描述已废弃
7. **长期记忆需精简**：MEMORY.md 含过期条目，需根据实际状态同步

治理目标：修复全部已知问题，重写 PRD，建立文档治理规则防止再次腐化。

---

## 执行步骤与结果

### 步骤 1：合并 `docs/plans/` → `docs/plan/` ✅

**问题**：项目约定计划目录为 `docs/plan/`，但 brainstorming skill 自动创建了 `docs/plans/`。

**操作**：
1. `git mv docs/plans/2026-03-02-permissions-observability.md docs/plan/`
2. `git mv docs/plans/2026-03-03-tray-menu-redesign.md docs/plan/`
3. 删除 `docs/plans/` 空目录

### 步骤 2：重写 PRD ✅

**问题**：旧 PRD（根目录 `prd.md`，1198 行）混合产品需求与实现细节，仍描述已废弃的 STT+LLM 两步管线。

**操作**：
1. `git mv prd.md docs/prd.md`
2. 以资深产品经理视角完全重写为现代精简 PRD（~380 行，8 节结构）
3. 竞品分析内容提取到 `docs/competitive-analysis.md`

**新 PRD 结构**：产品概要 → 成功指标 → 功能需求（含状态标注）→ 交互设计规格 → 异常处理与数据安全 → 平台策略 → 实现状态追踪 → 约束与风险

**从旧 PRD 删除的内容（已有权威来源）**：
- 技术选型推荐 → `docs/architecture/overview.md`
- 开发模块拆解 → `docs/modules/*.md`
- API 接口设计 → `src/shared/lib/types.ts` + `docs/modules/`
- 开发路线图 → `docs/plan/`
- 架构师审视 → ADRs

### 步骤 3：更新 `docs/README.md` 导航索引 ✅

**补充**：
- 架构区：`frontend.md`、`ui-design.md`
- 模块区：`jni-bridge.md`
- 计划区：Phase 5、两个新移入的 Sprint 实施计划
- 顶部：PRD 入口 + 竞品分析入口

**新增文档治理规则**：权威来源定义表 + Sprint 完成检查清单

### 步骤 4：修复根目录 `README.md` ✅

| 修改项 | 旧值 | 新值 |
|--------|------|------|
| 产品描述 | "语音识别 + LLM 智能润色" | "多模态 LLM 一步识别+润色" |
| 配置说明 | "灵活配置：自由选择 STT / LLM Provider" | "配置简单：仅需一个 DashScope API Key" |
| 框架版本 | "Tauri 2.0" | "Tauri 2.10" |
| UI 组件库 | 缺失 | 添加 Fluent UI 2 行 |
| 测试数量 | "117 Rust tests + 26 Frontend tests" | "222 自动化测试" |
| 安装包 | 缺 macOS | 添加 `.dmg — macOS` |

### 步骤 5：修复 `CLAUDE.md` 命令表 ✅

**新增命令**：`inject_text`、`get_recent_history`、`delete_history_batch`

**修正**：`clear_history` 返回值 `()` → `u64`；`stop_recording` 描述更新

**新增开发约定**：计划文档目录统一使用 `docs/plan/`，禁止 `docs/plans/`

### 步骤 6：补全 CHANGELOG.md ✅

从 git log 重建 11 个版本条目（v0.5.0 ~ v0.10.1），遵循 Keep a Changelog 格式。

### 步骤 7：精简 MEMORY.md ✅

- 音频编码更新：WAV → MP3 优先、WAV 回退
- 删除过细的 Sprint 实施记录，精简为一行摘要 + 文档引用
- 保留核心架构决策、用户偏好、代码红线、环境注意事项

### 步骤 8：持久化本治理计划 ✅

本文档即为持久化结果，保存于 `docs/plan/2026-03-03-documentation-governance.md`。

---

## 涉及文件

| 文件 | 变更类型 |
|------|----------|
| `docs/prd.md` | 从根目录移入 + 完全重写 |
| `docs/competitive-analysis.md` | 新建（从旧 PRD 提取） |
| `docs/README.md` | 导航更新 + 治理规则 |
| `README.md` | 6 处事实修正 |
| `CLAUDE.md` | 命令表修正 + 开发约定 |
| `CHANGELOG.md` | 补全 11 个版本 |
| `docs/plan/2026-03-02-permissions-observability.md` | 从 `plans/` 移入 |
| `docs/plan/2026-03-03-tray-menu-redesign.md` | 从 `plans/` 移入 |
| `docs/plan/2026-03-03-documentation-governance.md` | 新建（本计划） |
| `docs/plans/` | 删除 |

---

## 建立的治理规则

为防止文档再次腐化，本次治理在 `docs/README.md` 中建立了以下规则：

1. **权威来源定义表**：每类信息只有一个权威文件，跨文档引用不重复
2. **Sprint 完成检查清单**：每次迭代完成时必须核对文档同步
3. **目录命名约定**：`docs/plan/` 为唯一计划目录，禁止 `docs/plans/`
4. **文件命名规则**：Phase 计划用 `phase-N-*.md`，Sprint 实施计划用 `YYYY-MM-DD-*.md`
