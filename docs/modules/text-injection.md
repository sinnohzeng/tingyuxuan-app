# Text Injection & Context Detection

## 模块职责

文本注入模块负责将处理后的文本插入到用户光标所在位置，并检测当前上下文信息（活动窗口名称、选中文本）。该模块根据 Linux 显示服务器类型（X11 / Wayland）自动选择对应的 CLI 工具链。

**源文件:**

- `src-tauri/src/text_injector.rs` -- 文本注入逻辑
- `src-tauri/src/context.rs` -- 上下文检测逻辑

---

## 关键类型定义

### DisplayServer

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}
```

运行时枚举，标识当前 Linux 桌面使用的显示服务器协议。所有公开函数在执行前都会调用 `detect_display_server()` 以确定工具链。

---

## Public API

### 文本注入 (`text_injector.rs`)

| 函数 | 签名 | 说明 |
|------|------|------|
| `detect_display_server()` | `fn detect_display_server() -> DisplayServer` | 依次检查 `XDG_SESSION_TYPE`、`WAYLAND_DISPLAY`、`DISPLAY` 环境变量，返回对应的 DisplayServer 枚举值 |
| `inject_text(text)` | `fn inject_text(text: &str) -> Result<(), String>` | 主入口。根据文本长度选择注入策略（见下方策略表），根据 DisplayServer 选择工具链 |

**注入策略:**

| 条件 | 策略 | 说明 |
|------|------|------|
| `text.len() < 200` | 直接模拟键入 | X11: `xdotool type --clearmodifiers --`; Wayland: `wtype --` |
| `text.len() >= 200` | 剪贴板粘贴 | 保存当前剪贴板 -> 写入文本到剪贴板 -> 模拟 Ctrl+V -> 等待 100ms -> 恢复原始剪贴板 |

**各 DisplayServer 工具链:**

| DisplayServer | 直接键入 | 剪贴板读写 | 粘贴快捷键 |
|---------------|----------|-----------|-----------|
| X11 | `xdotool type --clearmodifiers --` | `xclip -selection clipboard` (stdin pipe) | `xdotool key --clearmodifiers ctrl+v` |
| Wayland | `wtype --` | `wl-copy` (stdin pipe) / `wl-paste` | `wtype -M ctrl v -m ctrl` |
| Unknown | `ydotool type --` | 不支持（仅直接键入） | 不支持 |

### 上下文检测 (`context.rs`)

| 函数 | 签名 | 说明 |
|------|------|------|
| `get_active_window_name()` | `fn get_active_window_name() -> Option<String>` | 获取当前焦点窗口的标题。X11: `xdotool getactivewindow getwindowname`; Wayland: `wlrctl toplevel find focused` |
| `get_selected_text()` | `fn get_selected_text() -> Option<String>` | 获取当前选中的文本。X11: `xclip -selection primary -o`; Wayland: `wl-paste --primary` |

---

## 错误处理策略

- **返回类型**: `Result<(), String>`（注入函数）和 `Option<String>`（上下文函数）
- **注入失败**: 将外部命令的错误信息格式化为 `String` 返回。调用方（`commands.rs`）通过 `tracing::error!` 记录日志但不会中断主流程
- **上下文检测失败**: 返回 `None`，静默降级。外部工具不存在或执行失败时不会产生 panic
- **剪贴板恢复失败**: 使用 `let _ =` 忽略错误，确保不因恢复失败阻塞主流程
- **输出编码**: 使用 `String::from_utf8_lossy()` 处理外部命令输出，非 UTF-8 字节被替换为 U+FFFD

---

## 测试覆盖

当前此模块 **没有自动化测试**。原因：

1. 所有功能依赖外部 CLI 工具（`xdotool`、`xclip`、`wtype` 等），需要运行中的显示服务器
2. `inject_text()` 会产生真实的键盘输入事件，无法在 CI 环境中安全执行
3. `detect_display_server()` 依赖环境变量，可以通过设置环境变量进行单元测试，但目前未实现

**建议的测试方向:**

- 对 `detect_display_server()` 编写单元测试（mock 环境变量）
- 对注入策略选择逻辑（长度阈值 200）编写单元测试
- 集成测试需在带有 display server 的环境中运行（如 Xvfb）

---

## 已知局限性

1. **仅支持 Linux**: 没有 macOS（`osascript`/`pbcopy`）或 Windows（`SendKeys`/`clip`）实现
2. **依赖外部 CLI 工具**: 需要用户安装 `xdotool`、`xclip`（X11）或 `wtype`、`wl-copy`、`wl-paste`（Wayland）以及可选的 `wlrctl`、`ydotool`。工具缺失时运行时报错
3. **同步阻塞**: `inject_text()` 在剪贴板路径下会 `std::thread::sleep(100ms)` 阻塞当前线程。在 Tauri async command 中调用时需注意
4. **无 UTF-8 验证**: 直接将 `text.as_bytes()` 写入 stdin，不验证文本是否包含对目标工具有问题的字符
5. **Wayland 支持不完整**: `wlrctl` 并非所有 Wayland compositor 都支持；`wtype` 需要 `wlr-virtual-keyboard-unstable-v1` 协议
6. **无控制字符过滤**: 注入文本中的制表符、换行符等控制字符会被原样传递给 `xdotool`/`wtype`，可能产生意外行为
7. **Unknown fallback 有限**: `ydotool` 仅支持直接键入，不支持剪贴板路径。且 `ydotool` 需要 root 权限或 `input` 组权限
8. **`--` 分隔符安全**: `xdotool type` 和 `wtype` 调用使用 `--` 分隔符防止文本被误解析为命令行参数。但剪贴板路径通过 stdin pipe 传输，本身是安全的
