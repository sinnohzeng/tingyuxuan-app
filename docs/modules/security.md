# Security

## 模块职责

本文档描述 TingYuXuan 应用的安全模型，涵盖内容安全策略（CSP）、API Key 存储、Tauri 权限能力、输入验证、文本注入安全性以及依赖安全等方面。

**相关文件:**

- `src-tauri/tauri.conf.json` -- Tauri 应用配置（含 CSP）
- `src-tauri/capabilities/default.json` -- Tauri 权限能力声明
- `src-tauri/src/commands.rs` -- API Key 管理（keyring 集成）+ 输入验证
- `src-tauri/src/text_injector.rs` -- 文本注入安全考量
- `crates/tingyuxuan-core/src/config.rs` -- 配置中的 key 引用机制

---

## Content Security Policy (CSP)

### 当前状态 -- 已加固（Phase 4 Step 3）

```json
// src-tauri/tauri.conf.json
"security": {
    "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
}
```

此策略确保：

- **脚本限制**: 只能从应用自身加载脚本（禁止 inline script 和外部脚本），有效防止 XSS 攻击
- **样式限制**: 允许内联样式（CSS-in-JS 框架需要 `'unsafe-inline'`），但禁止外部样式表
- **图片限制**: 仅允许自身资源和 `data:` URI
- **连接限制**: 仅允许自身和 Tauri IPC 通道（`ipc:` 和 `http://ipc.localhost`）

> **注意**: 所有外部 API 调用（STT/LLM）都通过 Rust 后端的 `reqwest` 发起，不经过 WebView 的 `fetch`，因此 `connect-src` 策略不影响核心功能。

---

## API Key 存储

### 架构

API Key 通过操作系统 keyring 安全存储，避免明文写入配置文件。

```
┌─────────────┐     keyring crate      ┌──────────────────┐
│ Tauri 前端   │ ──save_api_key()──────> │ OS Keyring       │
│             │ <──get_api_key()──────  │ (Secret Service) │
└─────────────┘                         └──────────────────┘
                                               │
                                        service: "tingyuxuan"
                                        username: "stt" | "llm"
```

### Keyring 配置

| 参数 | 值 |
|------|---|
| service | `"tingyuxuan"` |
| username (STT) | `"stt"` |
| username (LLM) | `"llm"` |

### Tauri Commands

| 命令 | 说明 | 输入验证 |
|------|------|---------|
| `save_api_key(service, key)` | 将 API key 存入 OS keyring | service 白名单校验 + key 长度限制 (512B) + null byte 检查 |
| `get_api_key(service)` | 从 OS keyring 读取 API key。keyring 不可用时返回 `None` | — |

### Key 解析流程 (`resolve_api_key`)

```
1. 尝试从 keyring 读取 (service="tingyuxuan", username=service_name)
   ├── 成功且非空 → 返回 key
   └── 失败 ↓
2. 检查 config.api_key_ref
   ├── 非空且不以 "@keyref:" 开头 → 视为明文 key，直接返回
   └── 空或以 "@keyref:" 开头 → 返回 None（key 未配置）
```

### `@keyref:` 前缀约定

配置文件中的 `api_key_ref` 字段使用 `@keyref:` 前缀表示"此 key 存储在 keyring 中"。`resolve_api_key()` 遇到此前缀时不会将其作为有效 key 使用，而是依赖 keyring 查询。

### Fallback 行为

当 OS keyring 不可用时（如无头服务器、没有 Secret Service daemon）：

- `get_api_key()` 记录 `tracing::warn!` 并返回 `None`
- `resolve_api_key()` 回退到 `config.api_key_ref` 中的明文值
- 明文 key 会被写入 `~/.config/tingyuxuan/TingYuXuan/config.json`

---

## Tauri Capabilities

### 当前权限声明

```json
// src-tauri/capabilities/default.json
{
    "identifier": "default",
    "windows": ["floating-bar", "settings"],
    "permissions": [
        "core:default",
        "core:event:default",
        "core:window:default",
        "global-shortcut:default",
        "shell:default"
    ]
}
```

### 权限说明

| 权限 | 用途 | 风险评估 |
|------|------|---------|
| `core:default` | Tauri 核心功能（invoke、资源访问） | 低 -- 必需 |
| `core:event:default` | 事件系统（前后端通信） | 低 -- 必需 |
| `core:window:default` | 窗口管理（创建、显示、隐藏） | 低 -- 必需 |
| `global-shortcut:default` | 全局快捷键注册 | 低 -- 核心功能 |
| `shell:default` | **Shell 命令执行** | **中 -- CSP 已限制 XSS 攻击面** |

### shell:default 风险（已缓解）

`shell:default` 权限允许前端 JavaScript 通过 Tauri Shell API 执行系统命令。在 CSP 已配置的当前状态下，XSS 攻击面已大幅缩小：只有应用自身的脚本可以执行。

**后续建议**: 移除 `shell:default`，改为在 Rust 后端执行所有系统命令（text injection 等），仅暴露特定的 Tauri commands。

---

## 输入验证

### 当前状态 -- 已加固（Phase 4 Step 3）

所有 Tauri commands 入口均添加了输入验证：

| 场景 | 验证措施 |
|------|---------|
| `inject_text(text)` | 长度限制 (50,000 字节) + null byte 检查 |
| `save_api_key(service, key)` | service 白名单 (`stt`, `llm`) + key 长度限制 (512B) + null byte 检查 |
| `start_recording(mode)` | `parse_mode()` 将未知值 fallback 到 Dictate（安全默认值） |
| `search_history(query, limit)` | 长度限制 (500 字节) + null byte 检查 |
| `add_dictionary_word(word)` | trim + 非空检查 + 长度限制 (100 字节) + null byte 检查 |
| `save_config(config)` | Serde 反序列化自动验证类型（后续可添加字段值验证） |

### 验证常量

```rust
const MAX_INJECT_TEXT_LEN: usize = 50_000;
const MAX_API_KEY_LEN: usize = 512;
const MAX_SEARCH_QUERY_LEN: usize = 500;
const MAX_DICT_WORD_LEN: usize = 100;
const VALID_KEY_SERVICES: &[&str] = &["stt", "llm"];
```

### 辅助函数

- `check_no_null_bytes(s, field_name)` — 拒绝包含 `\0` 的字符串
- `check_max_len(s, max, field_name)` — 拒绝超过字节长度限制的字符串

---

## 文本注入安全性

### 命令行注入防护

| 场景 | 防护措施 | 安全等级 |
|------|---------|---------|
| 直接键入（短文本） | `xdotool type --clearmodifiers -- text`，`--` 分隔符防止 text 被解析为参数 | 安全 |
| 直接键入（Wayland） | `wtype -- text`，同样使用 `--` 分隔符 | 安全 |
| 剪贴板路径 | 通过 stdin pipe 将文本写入 `xclip`/`wl-copy`，不经过命令行参数 | 安全 |
| ydotool fallback | `ydotool type -- text`，使用 `--` 分隔符 | 安全 |

### 控制字符过滤（Phase 4 Step 3 新增）

直接键入路径（非剪贴板路径）的文本在注入前经过 `sanitize_for_typing()` 过滤：

| 字符类型 | 处理 |
|----------|------|
| 普通可见字符 | 保留 |
| `\n` (换行) | 保留 -- 合法的文本内容 |
| `\t` (制表符) | 保留 -- 合法的文本内容 |
| `\0` (null) | **移除** -- 可导致 C 库未定义行为 |
| `\x08` (退格) | **移除** -- 可删除已有内容 |
| `\x1b` (escape) | **移除** -- 可触发终端 escape 序列 |
| `\x7f` (DEL) | **移除** -- 可删除已有内容 |
| 其他控制字符 (0x00-0x1F) | **移除** -- 可能产生意外行为 |

### 已知限制

1. **剪贴板竞争**: 在 100ms 恢复延迟期间，用户或其他程序可能已经使用了被替换的剪贴板内容
2. **换行符在终端中**: 保留的 `\n` 在终端模拟器中仍然可能触发命令执行（这是用户主动输入的场景，非安全漏洞）

### 测试覆盖

6 个单元测试覆盖 `sanitize_for_typing()` 和 `detect_display_server()`：
- 正常文本保留
- 换行符和制表符保留
- null byte 和退格移除
- escape 和 DEL 移除
- bell 和 form feed 移除
- 无显示环境检测

---

## 依赖安全

### 关键依赖

| 依赖 | 用途 | 安全特性 |
|------|------|---------|
| `rusqlite` (bundled feature) | SQLite 数据库 | 使用 bundled 特性编译自带 SQLite，避免依赖系统 SQLite 版本（可能过旧或有已知漏洞） |
| `reqwest` | HTTP 客户端（STT/LLM API 调用） | 默认启用 TLS（rustls 或 native-tls），所有 API 调用通过 HTTPS |
| `keyring` | OS keyring 访问 | 使用操作系统原生 Secret Service（Linux: gnome-keyring/kwallet） |
| `serde` / `serde_json` | 序列化 | 纯 Rust 实现，无 C 依赖，内存安全 |
| `tauri` | 应用框架 | 提供进程隔离、IPC、capability 系统 |

### 外部 CLI 工具

| 工具 | 安全考量 |
|------|---------|
| `xdotool` | 以当前用户权限运行，可模拟键盘输入到任何窗口 |
| `xclip` | 访问 X11 剪贴板，任何 X11 应用都可以读取 |
| `wtype` | 需要 Wayland compositor 支持 `wlr-virtual-keyboard` 协议 |
| `wl-copy`/`wl-paste` | Wayland 剪贴板工具 |
| `ydotool` | 需要 root 权限或 `input` 组，直接操作 `/dev/uinput` |

---

## 已修复的安全问题（Phase 4 Step 3）

### 1. CSP 从 null 加固为严格策略 ✅

CSP 已从 `null` 修改为严格策略，禁止外部脚本加载和 inline script 执行。

### 2. Tauri commands 已添加输入验证 ✅

所有关键 Tauri commands 入口已添加长度限制、null byte 检查和参数白名单校验。

### 3. 文本注入已添加控制字符过滤 ✅

`sanitize_for_typing()` 在直接键入路径过滤危险控制字符，保留合法的换行和制表符。

### 4. shell:default 权限 -- 待后续处理

`shell:default` 仍然存在，但 CSP 已大幅缩小攻击面。建议在后续版本中移除，将所有 shell 调用迁移到 Rust 后端。
