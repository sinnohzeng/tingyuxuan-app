# 前端架构

> 本文档描述 React 前端的目录结构、路由分层、状态管理和错误处理链路。

## 目录结构

采用 **feature-based** 组织方式，每个功能模块自包含视图、hooks 和测试：

```
src/
├── features/
│   ├── dashboard/          首页 + 统计卡片
│   │   ├── HomePage.tsx
│   │   ├── StatsGrid.tsx
│   │   └── *.test.tsx
│   ├── dictionary/         词典管理
│   │   ├── DictionaryPage.tsx
│   │   └── hooks/useDictionary.ts
│   ├── history/            历史记录
│   │   ├── HistoryPage.tsx
│   │   ├── RecentTranscripts.tsx
│   │   └── hooks/useHistory.ts
│   ├── onboarding/         引导流程
│   │   ├── OnboardingFlow.tsx
│   │   ├── IntroSlide.tsx
│   │   ├── SetupWizard.tsx
│   │   └── PermissionGuide.tsx
│   ├── recording/          录音浮动条 + 结果面板
│   │   ├── FloatingBar.tsx
│   │   └── ResultPanel.tsx
│   └── settings/           设置对话框
│       ├── SettingsDialog.tsx
│       ├── sections/       ApiSection、GeneralSection 等
│       └── hooks/          useApiKey、useConnectionTest、useConfig
├── shared/
│   ├── components/         MainLayout、ToastHost
│   ├── hooks/              useTauriEvent
│   ├── lib/                types.ts、logger.ts、theme.ts
│   └── stores/             appStore、uiStore、statsStore
├── App.tsx                 路由定义
├── main.tsx                入口
└── test-utils.tsx          测试工具（renderWithProviders + resetStores）
```

## 路由分层

基于 React Router v7，两层路由：

```
/onboarding          → OnboardingFlow（引导流程，首次启动）
/main                → MainLayout（主布局容器）
  /main              → HomePage（首页仪表盘）
  /main/history      → HistoryPage
  /main/dictionary   → DictionaryPage
```

- `MainLayout` 负责侧边导航 + Outlet + Tauri 事件监听（open-settings、open-history）
- 首次启动检测：`localStorage.getItem("onboarding_complete")` 同步判断，未完成则重定向到 `/onboarding`
- 设置面板以 Fluent Dialog 形式叠加在当前页面上（不占路由）

## 状态管理

三个 Zustand store，各司其职：

| Store | 文件 | 职责 | 持久化 |
|-------|------|------|--------|
| appStore | `shared/stores/appStore.ts` | 录音状态、管线状态、Tauri 事件 | 否 |
| uiStore | `shared/stores/uiStore.ts` | 设置面板开关、Toast 队列 | 否 |
| statsStore | `shared/stores/statsStore.ts` | 仪表盘统计、60s 缓存 | 否 |

**数据流向**：Tauri 事件 → appStore 更新 → React 组件订阅渲染

## 错误处理链路

```
hook catch 块
    │
    ├── log.error("技术细节", e)      ← createLogger 结构化日志
    │
    └── uiStore.showToast({           ← 用户可见通知
          type: "error",
          title: "用户可理解的消息"
        })
```

**约定**：
- 禁止静默 `catch {}`，所有错误必须通知用户
- 禁止裸 `console.error`，统一使用 `createLogger` 工厂
- Toast 消息用中文，面向用户；log 记录技术细节，面向开发者
- 乐观更新失败时执行回滚（如 useDictionary 的 addWord/removeWord）

## Tauri 事件监听

通过 `useTauriEvent` 自定义 hook 封装：

```ts
useTauriEvent("open-settings", openSettings);
useTauriEvent("open-history", navigateToHistory);
```

hook 内部实现 mounted 守卫，防止异步 `listen()` 在组件卸载后 resolve 导致监听器泄漏。

## 测试策略

- 依赖 Fluent UI 主题 token 的组件（HomePage、SettingsDialog 等）使用 `renderWithProviders`（FluentProvider + MemoryRouter）
- Tauri `invoke` 通过 `vi.hoisted()` + `vi.mock("@tauri-apps/api/core")` 统一 mock
- 每个 `beforeEach` 调用 `resetStores()` 重置所有 Zustand store
