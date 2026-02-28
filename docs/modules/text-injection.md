# Text Injection & Context Detection

## 模块职责

文本注入模块负责将处理后的文本插入到用户光标所在位置，并检测当前上下文信息（活动窗口名称、选中文本等）。通过平台抽象层（见 [ADR-0006](../architecture/adr/0006-platform-abstraction-layer.md)），各平台使用原生 API 或最合适的工具链实现相同的 trait 接口。

**源文件:**

```
src-tauri/src/platform/
├── mod.rs        # TextInjector + ContextDetector trait 定义、类型别名、共享工具函数
├── error.rs      # PlatformError (thiserror)、PermissionStatus 枚举
├── linux.rs      # LinuxTextInjector + LinuxContextDetector
├── macos.rs      # MacOSTextInjector + MacOSContextDetector + FnKeyMonitor
└── windows.rs    # WindowsTextInjector + WindowsContextDetector
```

---

## 核心 Trait

```rust
pub trait TextInjector {
    fn inject_text(&self, text: &str) -> Result<(), PlatformError>;
}

pub trait ContextDetector {
    fn collect_context(&self) -> InputContext;
}
```

通过编译时类型别名实现零开销分发：

```rust
#[cfg(target_os = "linux")]   pub type PlatformInjector = linux::LinuxTextInjector;
#[cfg(target_os = "macos")]   pub type PlatformInjector = macos::MacOSTextInjector;
#[cfg(target_os = "windows")] pub type PlatformInjector = windows::WindowsTextInjector;
```

---

## 文本注入策略

所有平台共享统一的自适应策略：**短文本直接键入、长文本剪贴板粘贴**。

| 条件 | 策略 | 说明 |
|------|------|------|
| `text.len() <= 200` | 直接键入 | 逐字符/逐块模拟键盘输入，不扰动剪贴板 |
| `text.len() > 200` | 剪贴板粘贴 | 保存剪贴板 → 写入文本 → 粘贴 → 等待 100ms → 恢复原始剪贴板 |

### 各平台实现

| 平台 | 直接键入 | 剪贴板读写 | 粘贴快捷键 |
|------|---------|-----------|-----------|
| **Linux X11** | `xdotool type --clearmodifiers --` | `xclip -selection clipboard` (stdin pipe) | `xdotool key --clearmodifiers ctrl+v` |
| **Linux Wayland** | `wtype --` | `wl-copy` (stdin pipe) / `wl-paste` | `wtype -M ctrl v -m ctrl` |
| **macOS** | `CGEvent::set_string()` (每 20 个 UTF-16 code unit 一块) | `arboard` crate（原生 NSPasteboard） | `CGEvent` 模拟 Cmd+V |
| **Windows** | `SendInput` + `KEYEVENTF_UNICODE` (批量 syscall) | Win32 `OpenClipboard` + `CF_UNICODETEXT` | `SendInput` 模拟 Ctrl+V |

### 输入预处理

所有平台在注入前调用 `sanitize_for_typing(text)` 预处理文本，过滤可能干扰键盘模拟的控制字符。

---

## 上下文检测

### InputContext 结构

```rust
pub struct InputContext {
    pub app_name: Option<String>,         // 当前应用名称
    pub window_title: Option<String>,     // 当前窗口标题
    pub clipboard_text: Option<String>,   // 剪贴板内容
    pub selected_text: Option<String>,    // 选中文本
    pub app_package: Option<String>,      // Android 包名（桌面端 None）
    pub browser_url: Option<String>,      // 浏览器 URL（需扩展，后续迭代）
    // ... 其他扩展字段
}
```

### 各平台采集方式

| 信号 | Linux | macOS | Windows |
|------|-------|-------|---------|
| **应用名称** | `xdotool getactivewindow getwindowclassname` (X11) / 窗口标题 (Wayland) | AXUIElement: `AXFocusedApplication` → `AXTitle` | `GetForegroundWindow` + `GetWindowTextW` |
| **窗口标题** | `xdotool getactivewindow getwindowname` (X11) / `wlrctl toplevel find focused` (Wayland) | AXUIElement: `AXFocusedWindow` → `AXTitle` | `GetForegroundWindow` + `GetWindowTextW` |
| **选中文本** | `xclip -selection primary -o` (X11) / `wl-paste --primary` (Wayland) | AXUIElement: `AXFocusedUIElement` → `AXSelectedText`；fallback: 模拟 Cmd+C | 模拟 Ctrl+C + 读剪贴板 |
| **剪贴板** | `xclip -selection clipboard -o` / `wl-paste` | `arboard` crate 读取 | Win32 `OpenClipboard` + `CF_UNICODETEXT` |
| **并发策略** | 4 项并行（`thread::scope`，每项 200ms 超时） | 全同步顺序执行（AXUIElement <1ms） | 全同步顺序执行 |

### macOS 上下文采集的优势 (v0.7.0)

macOS 使用 AXUIElement API 直接查询，相比 Linux/Windows 的 Cmd+C / Ctrl+C 模拟方式：
- **不扰动剪贴板**：`AXSelectedText` 直接读取选中文本
- **无竞态条件**：`selected_text` 和 `clipboard_text` 采集不再冲突
- **极低延迟**：每项查询 <1ms（vs Linux 每项 ~200ms）

---

## 权限管理 (macOS)

macOS 需要两个独立权限才能正常工作：

| 权限 | 用途 | 检测 API |
|------|------|---------|
| **Accessibility（辅助功能）** | CGEvent 模拟按键、AXUIElement 查询 | `AXIsProcessTrusted()` |
| **Input Monitoring（输入监控）** | CGEventTap 监听 Fn 键 | `CGPreflightListenEventAccess()` |

权限状态通过四值枚举返回：

```rust
pub enum PermissionStatus {
    Granted,
    AccessibilityRequired,
    InputMonitoringRequired,
    BothRequired,
}
```

前端 `PermissionGuide` 组件每 2 秒自动轮询权限状态，用户授权后即时更新 UI。

---

## 错误处理

- **统一错误类型**: `PlatformError`（thiserror 派生）
  - `InjectionFailed(String)` — 文本注入失败
  - `ClipboardError(String)` — 剪贴板操作失败
  - `ToolNotFound { tool: String }` — 外部工具缺失（仅 Linux）
  - `PermissionDenied { permission: String }` — 权限不足（仅 macOS）
- **上下文检测**: 返回 `Option<String>`，静默降级
- **剪贴板恢复**: 尽力而为（`if let Err(e) = ...`），不阻塞主流程

---

## 测试覆盖

| 测试类型 | 覆盖范围 |
|----------|---------|
| 常量一致性测试 | `DIRECT_INPUT_THRESHOLD == 200`、`MAX_UNICODE_PER_EVENT == 20` |
| UTF-16 分块测试 | ASCII、CJK、Emoji、混合文本的分块正确性 |
| Linux 环境检测测试 | `detect_display_server()` 环境变量 mock |
| macOS 集成测试 (`#[cfg(target_os = "macos")]`) | 剪贴板读写、CGEvent 直接输入、AXUIElement 查询、权限检测（均为 no-panic 测试） |

> **注意**：文本注入和上下文检测的完整功能测试需要 GUI 环境（显示服务器 / 窗口管理器），无法在 headless CI 中运行。macOS 集成测试使用 `let _ =` 仅验证不 panic。
