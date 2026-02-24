# Security

## 模块职责

本文档描述 TingYuXuan 应用的安全模型，涵盖内容安全策略（CSP）、API Key 存储、Tauri 权限能力、输入验证、文本注入安全性以及依赖安全等方面。

**相关文件:**

- `src-tauri/tauri.conf.json` -- Tauri 应用配置（含 CSP）
- `src-tauri/capabilities/default.json` -- Tauri 权限能力声明
- `src-tauri/src/commands.rs` -- API Key 管理（keyring 集成）
- `src-tauri/src/text_injector.rs` -- 文本注入安全考量
- `crates/tingyuxuan-core/src/config.rs` -- 配置中的 key 引用机制

---

## Content Security Policy (CSP)

### 当前状态 -- 存在漏洞

```json
// src-tauri/tauri.conf.json
"security": {
    "csp": null
}
```

**风险等级: 高**

`csp: null` 表示没有任何内容安全策略限制。WebView 中的页面可以：

- 加载并执行任意来源的脚本（inline script、外部 script）
- 连接任意外部服务器
- 加载任意来源的资源（图片、字体、样式）

这使得应用面临 XSS（跨站脚本攻击）风险。若恶意内容进入 WebView（例如通过 AI 返回的文本未经转义直接渲染），可在 Tauri 上下文中执行任意 JavaScript。

### 计划修复（Phase 4 Step 3）

```
default-src 'self';
script-src 'self';
style-src 'self' 'unsafe-inline';
img-src 'self' data:;
connect-src 'self' ipc: http://ipc.localhost
```

此策略将：

- 限制脚本只能从应用自身加载（禁止 inline script 和外部脚本）
- 允许内联样式（框架需要）
- 限制图片来源为自身和 data URI
- 限制网络连接为自身和 Tauri IPC 通道

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

| 命令 | 说明 |
|------|------|
| `save_api_key(service, key)` | 将 API key 存入 OS keyring |
| `get_api_key(service)` | 从 OS keyring 读取 API key。keyring 不可用时返回 `None` |

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
| `shell:default` | **Shell 命令执行** | **高 -- 过于宽泛** |

### shell:default 风险

`shell:default` 权限允许前端 JavaScript 通过 Tauri Shell API 执行系统命令。在 CSP 为 null 的当前配置下，这意味着 XSS 攻击可以直接获得系统命令执行能力。

**建议**: 移除 `shell:default`，改为在 Rust 后端执行所有系统命令（text injection 等），仅暴露特定的 Tauri commands。

---

## 输入验证

### 当前状态 -- 最小化验证

Tauri commands 层面的输入验证极少：

| 场景 | 当前验证 | 缺失的验证 |
|------|---------|-----------|
| `inject_text(text)` | 无 | 无长度限制、无 null byte 检查、无控制字符过滤 |
| `save_api_key(service, key)` | 无 | service 参数无白名单校验 |
| `start_recording(mode)` | `parse_mode()` 将未知值 fallback 到 Dictate | 不算严格验证 |
| `search_history(query, limit)` | 无 | 无长度限制（长 query 的 LIKE 可能较慢） |
| `add_dictionary_word(word)` | `trim().is_empty()` 检查 | 无长度限制、无特殊字符过滤 |
| `save_config(config)` | 无 | 无字段值验证（URL 格式、快捷键格式等） |

### 计划改进（Phase 4 Step 3）

- 对所有 Tauri command 参数添加长度限制
- 对 text injection 输入添加 null byte 和控制字符检查
- 对 service 参数添加白名单校验
- 对配置值添加格式验证

---

## 文本注入安全性

### 命令行注入防护

| 场景 | 防护措施 | 安全等级 |
|------|---------|---------|
| 直接键入（短文本） | `xdotool type --clearmodifiers -- text`，`--` 分隔符防止 text 被解析为参数 | 安全 |
| 直接键入（Wayland） | `wtype -- text`，同样使用 `--` 分隔符 | 安全 |
| 剪贴板路径 | 通过 stdin pipe 将文本写入 `xclip`/`wl-copy`，不经过命令行参数 | 安全 |
| ydotool fallback | `ydotool type -- text`，使用 `--` 分隔符 | 安全 |

### 未解决的安全问题

1. **无控制字符过滤**: 注入文本中的制表符（`\t`）、换行符（`\n`）、退格符（`\b`）等控制字符会被原样传递。恶意构造的文本可能：
   - 通过换行符在终端模拟器中执行命令
   - 通过制表符触发自动补全
   - 通过退格符删除已有内容

2. **无长度限制**: 超长文本可能导致 `xdotool`/`wtype` 进程卡死或内存耗尽

3. **剪贴板竞争**: 在 100ms 恢复延迟期间，用户或其他程序可能已经使用了被替换的剪贴板内容

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

## 已知漏洞（待 Phase 4 修复）

### 1. CSP 为 null -- 允许内联脚本注入

**严重程度:** 高

CSP 未设置，WebView 中可以执行任意 JavaScript。结合 `shell:default` 权限，XSS 可直接获得系统命令执行能力。

**修复计划:** Phase 4 Step 3 -- 设置严格的 CSP 策略。

### 2. Tauri commands 无输入验证

**严重程度:** 中

所有 Tauri commands 不验证输入参数的长度、格式和内容。恶意前端代码可以：

- 传入超长文本导致资源耗尽
- 传入包含 null byte 的字符串
- 传入非预期的 service 名称操作 keyring

**修复计划:** Phase 4 Step 3 -- 添加统一的输入验证层。

### 3. 文本注入无控制字符过滤

**严重程度:** 中

注入到用户光标位置的文本未经控制字符过滤。来自 LLM 的响应文本中如果包含控制字符，可能在目标应用中产生意外行为。

**修复计划:** Phase 4 Step 3 -- 在 `inject_text()` 入口添加控制字符白名单过滤。

### 4. shell:default 权限过于宽泛

**严重程度:** 中

`shell:default` 允许前端调用系统 shell。应改为仅在 Rust 后端执行系统命令，通过 Tauri commands 暴露受控接口。

**修复计划:** Phase 4 Step 3 -- 移除 `shell:default`，确认所有 shell 调用已迁移到 Rust 后端。
