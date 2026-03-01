# Phase 5: UI 大改造 — 对标 Typeless 全面升级

> 状态：✅ 已完成 | 开始：2026-03-01 | 完成：2026-03-01

## 目标

将听雨轩从「托盘驻留 + 悬浮录音条」的工具型应用升级为完整桌面应用：
左侧导航、首页仪表盘、历史记录、词典管理、设置弹窗。

## 用户确认的决策

| 决策项 | 选择 |
|--------|------|
| 设计体系 | Fluent 2 (@fluentui/react-components v9) |
| 页面范围 | 完整复刻 Typeless（首页+历史+词典+设置弹窗） |
| 首页统计 | 个性化 %、总时间、字数、节省时间、平均速度 |
| 引导流程 | 产品介绍页（可跳过）→ API 配置 → 权限检查 → 进入首页 |
| 启动行为 | 弹出主窗口，关闭缩小到托盘（可配置） |
| 架构 | 离线优先，导入导出代替云同步 |

## 架构决策记录

| # | 决策 | 理由 |
|---|------|------|
| AD-1 | 聚合查询替代物化统计表 | `INSERT OR REPLACE` 触发器误触发；<10 万行聚合 <5ms；statsStore 60s 缓存 |
| AD-2 | Toast 通知替代静默错误吞没 | 6 个 hook 静默吞错；Rust 端有 StructuredError，前端需对等反馈 |
| AD-3 | useTauriEvent hook + mounted 守卫 | 异步 listen() 在 cleanup 后 resolve 导致泄漏；hook 消除重复样板 |
| AD-4 | localStorage 引导状态 | is_first_launch 语义不准确（检查 pipeline）；localStorage 同步判断避免闪烁 |
| AD-5 | close-to-tray 可配置化 | AppConfig.general.minimize_to_tray 控制，默认 true |
| AD-6 | SetupWizard 组合复用 ApiSection | 复用 ApiSection（109 行），不重建 API 配置 UI，遵循 DRY |

## Sprint 完成状态

### Sprint 1: 基础设施 ✅

依赖安装、Tauri 窗口配置、feature-based 目录重组、react-router-dom 路由改造、Fluent 主题、共享 stores。

### Sprint 2: 主窗口页面 + 设置弹窗 ✅

29 个新文件（1,836 行）：hooks、SettingsDialog、设置分区、MainLayout、首页仪表盘、历史记录、词典管理。清理旧 Settings/ 9 个孤立文件。Toast 错误通知系统替代 8 个文件静默 catch。ToastHost useRef 稳定化 + useHistory 错误模式标准化。

### Sprint 3: Rust 统计引擎 ✅

AggregateStats 聚合查询 + get_dashboard_stats 命令 + 5 个 Rust 测试。全列 COALESCE 防空表 NULL。rusqlite 0.38 不支持 u64 FromSql，用 i64 后 cast。

### Sprint 4: 窗口管理 ✅

- 修复 tray.rs 窗口标签（`"settings"` → `"main"`）
- 托盘双击显示/聚焦主窗口（Windows/macOS）
- 关闭缩小到托盘（可配置：`AppConfig.general.minimize_to_tray`）
- `useTauriEvent` 自定义 hook（mounted 守卫）
- MainLayout 事件监听（open-settings、open-history）

### Sprint 5: 引导流程 ✅

- IntroSlide：3 屏产品介绍 + a11y（role="tablist", aria-selected）
- SetupWizard：组合复用 ApiSection（DRY），~35 行胶水代码
- PermissionGuide：按钮触发检查（非 setInterval 轮询），Windows/Linux 自动跳过
- OnboardingFlow：ProgressBar 步骤进度 + FluentProvider 主题
- 首次启动检测：localStorage("onboarding_complete") 同步判断

### Sprint 6: 测试 + 文档 + 收尾 ✅

- test-utils.tsx：renderWithProviders + resetStores（appStore + uiStore + statsStore）
- 8 个新测试文件（29 个测试用例），总计 71 前端测试
- console.error → createLogger 标准化（7 个文件）
- DDD 文档同步：frontend.md、ui-design.md、CLAUDE.md 更新

## 验证结果

| 验证项 | 结果 |
|--------|------|
| Rust tests | 122 通过 |
| Frontend tests | 71 通过 |
| tsc --noEmit | 零错误 |
| 静默 catch | 零匹配 |
| 裸 console.error | 仅 test-setup.ts |
