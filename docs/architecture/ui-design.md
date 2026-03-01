# UI 设计体系

> 本文档描述前端 UI 组件库选型、主题系统和通知架构。

## Fluent UI 2 集成

选用 Microsoft Fluent UI 2（`@fluentui/react-components`）作为组件库：

- **一致性**：与 Windows 11 原生风格统一，提升桌面应用质感
- **主题系统**：内置亮色/暗色主题 token，支持运行时切换
- **可访问性**：WCAG 2.1 AA 合规，内置键盘导航和 ARIA 属性

### 使用的核心组件

| 组件 | 用途 |
|------|------|
| FluentProvider | 主题上下文提供者 |
| Button | 各类按钮（primary/secondary/subtle） |
| Input | 文本输入框 |
| Dialog | 设置面板模态框 |
| TabList/Tab | 设置页 Tab 切换 |
| Card | 统计卡片、引导卡片 |
| Tag | 词典词汇标签 |
| ProgressBar | 引导步骤进度 |
| Spinner | 加载态指示器 |
| Select/Option | 下拉选择（提供商选择） |
| Toaster/Toast | 通知系统（通过 ToastHost 桥接） |

## 主题切换

```
useSystemTheme() hook
    │
    ├── matchMedia("(prefers-color-scheme: dark)")
    │
    ├── 返回 webLightTheme 或 webDarkTheme
    │
    └── FluentProvider theme={theme}
```

- 跟随系统偏好自动切换，无需手动配置
- `shared/lib/theme.ts` 封装 `useSystemTheme` hook
- 所有颜色通过 Fluent token 引用，不使用硬编码色值

## Toast 通知架构

```
hook catch 块
    │
    └── uiStore.showToast({ type, title })
            │
            ├── toasts[] 队列更新
            │
            └── ToastHost 组件订阅
                    │
                    └── Fluent useToastController().dispatchToast()
                            │
                            └── Fluent Toaster 渲染通知
```

### 设计要点

- **解耦**：业务逻辑（hooks/stores）不直接依赖 Fluent Toaster API，通过 uiStore 中转
- **队列化**：多个错误不会互相覆盖，按顺序显示
- **ToastHost** 使用 `useRef` 稳定化回调，避免 `useEffect` 无限循环
- 支持类型：`error`（红色）、`warning`（黄色）、`success`（绿色）、`info`（蓝色）

## 引导流程 UX

```
首次启动
    │
    └── /onboarding 路由
            │
            ├── IntroSlide（3 屏产品介绍）
            │     └── 步骤指示器 + 跳过/下一步
            │
            ├── SetupWizard（API 配置）
            │     └── 复用 ApiSection 组件（DRY）
            │
            ├── PermissionGuide（macOS 权限）
            │     └── 按钮触发检查（非轮询）
            │
            └── localStorage("onboarding_complete") = "1"
                    │
                    └── navigate("/main")
```

- Windows/Linux：PermissionGuide 自动跳过（`check_platform_permissions` 返回 `"granted"`）
- a11y：IntroSlide 步骤指示器使用 `role="tablist"` + `aria-selected` + `aria-label`
