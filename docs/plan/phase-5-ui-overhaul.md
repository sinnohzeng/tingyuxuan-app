# Phase 5: UI 大改造 — 对标 Typeless 全面升级

> 状态：进行中 | 开始：2026-03-01 | Sprint 1 ✅ 已完成

## 目标

将听雨轩从「托盘驻留 + 悬浮录音条」的工具型应用升级为完整桌面应用：
左侧导航、首页仪表盘、历史记录、词典管理、设置弹窗。

## 用户确认的决策

| 决策项 | 选择 |
|--------|------|
| 设计体系 | Fluent 2 (@fluentui/react-components v9) |
| 页面范围 | 完整复刻 Typeless（首页+历史+词典+设置弹窗） |
| 首页统计 | 个性化 %、总时间、字数、节省时间、平均速度 |
| 引导流程 | 产品介绍页（可跳过）→ API 配置 → 进入首页 |
| 启动行为 | 弹出主窗口，关闭缩小到托盘 |
| 架构 | 离线优先，导入导出代替云同步 |

## 架构决策摘要

- **A1 样式策略**：主窗口 Fluent 2 + Griffel；FloatingBar 保留 Tailwind
- **A2 目录结构**：Feature-based（`src/features/` + `src/shared/`）
- **A3 统计引擎**：SQLite 触发器 + 物化统计表，O(1) 读取
- **A4 无障碍**：aria-label、focus trap、keyboard navigation
- **A5 代码分割**：FloatingBar 静态 import，其余 React.lazy
- **A6 窗口持久化**：tauri-plugin-store 记忆窗口位置

详见工作计划文件中的完整架构决策。

## Sprint 概览

### Sprint 1: 基础设施 ✅

依赖安装、Tauri 窗口配置、feature-based 目录重组、react-router-dom 路由改造、Fluent 主题、共享 stores。

### Sprint 2: 主窗口页面 + 设置弹窗（当前）

29 个文件，~1,825 行，8 个 Wave：
1. Hooks（useConfig, useApiKey, useConnectionTest, useStats, useHistory, useDictionary）
2. SettingsDialog 框架 + AccountTab + AboutTab + PersonalizationTab
3. Settings Sections（Shortcut, Language, Audio, Behavior, ApiSection + 可复用组件）
4. MainLayout 集成（SettingsDialog + 首次启动检测 + 托盘事件）
5. 首页仪表盘（StatsCard, StatsGrid, RecentTranscripts, HomePage）
6. 历史记录（HistoryItem, HistoryList, HistoryPage）
7. 词典管理（WordTagGrid, DictionaryPage）
8. 清理旧 Settings 文件 + 全量验证

### Sprint 3: Rust 后端

stats 表 + 触发器 + `get_dashboard_stats` 命令 + CSV/JSON 导入导出。

### Sprint 4: 窗口管理

关闭缩小到托盘 + 托盘菜单指向 main + 双击弹出 + FloatingBar 引用修正。

### Sprint 5: 引导流程

介绍页（可跳过）+ SetupWizard Fluent 化 + PermissionGuide Fluent 化。

### Sprint 6: 测试 + 文档 + 收尾

测试包装器 + DDD 文档同步 + 代码质量验证。
