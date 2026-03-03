# 文档自动化工具链 + 文档分层体系

> 状态：✅ 已完成 | 执行日期：2026-03-03

## 背景

上一轮文档治理修复了文档内容层面的问题（PRD 重写、CHANGELOG 补全、目录合并等）。本次实施治理中提出的三条"未来方向"建议，并建立行业标准的文档分层体系。

## 实施内容

### 1. markdownlint-cli2 — 文档格式检查

- **工具**：markdownlint-cli2（npm devDependency）
- **配置**：`.markdownlint.json` + `.markdownlint-cli2.jsonc`
- **关键规则**：行长 200（中文友好）、代码块/表格豁免、允许重复兄弟标题（CHANGELOG 兼容）
- **命令**：`npm run lint:docs`

### 2. lychee — 文档链接校验

- **工具**：lychee（Rust 二进制，CI 中用 GitHub Action）
- **配置**：`.lychee.toml`（排除 ms-settings/localhost/限流站点）
- **命令**：`npm run lint:links`（本地需安装 lychee）

### 3. git-cliff — CHANGELOG 自动生成

- **工具**：git-cliff（Rust 二进制，CI 中用 GitHub Action）
- **配置**：`cliff.toml`（中文分组：新增/修复/文档/性能/重构/测试/CI/杂项）
- **策略**：保留现有手写 CHANGELOG，git-cliff 仅用于本地预览和 Release 自动生成
- **命令**：`npm run changelog:next`（本地需安装 git-cliff）

### 4. CI 集成

- **ci.yml**：新增 `docs-check` job（markdownlint + lychee），与现有 job 并行
- **release.yml**：`create-release` job 新增 `orhun/git-cliff-action@v4`，替代 `generate_release_notes`

### 5. 文档分层体系

采用 Diátaxis 框架三层分类，不做目录重构：

| 层级 | 受众 | 目录 |
|------|------|------|
| 用户文档 | 终端用户 | `docs/guides/` |
| 开发者文档 | 贡献者/维护者 | `docs/architecture/`、`docs/modules/`、`CONTRIBUTING.md` |
| 内部文档 | 项目自身 | `docs/plan/`、`CLAUDE.md` |

- 新建 `CONTRIBUTING.md`：开发者入口（环境搭建、工作流、代码标准）
- `docs/README.md` 导航按三层重新分类
- `ci-release-notes.md` 从"用户指南"归类到"开发者文档"

## 涉及文件

| 文件 | 操作 |
|------|------|
| `.markdownlint.json` | 新建 |
| `.markdownlint-cli2.jsonc` | 新建 |
| `.lychee.toml` | 新建 |
| `cliff.toml` | 新建 |
| `CONTRIBUTING.md` | 新建 |
| `package.json` | 修改（devDep + 3 scripts） |
| `.github/workflows/ci.yml` | 修改（+docs-check job） |
| `.github/workflows/release.yml` | 修改（+git-cliff step） |
| `docs/README.md` | 修改（分层标签 + 检查清单） |
| `README.md` | 修改（+贡献入口） |
| `CLAUDE.md` | 修改（+新命令） |
| `docs/modules/audio.md` | 修改（行长修复） |
| `docs/modules/llm.md` | 修改（行长 + 代码 span 修复） |
