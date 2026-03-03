# 托盘菜单全面对标 Typeless + 麦克风设备选择

> **日期**：2026-03-03
> **状态**：实施中
> **关联 ADR**：[ADR-0009 音频设备选择](../architecture/adr/0009-audio-device-selection.md)

## Context

当前托盘右键菜单以"快捷录音操作"为主（听写/翻译/AI助手），缺少常用工具入口。对标 Typeless 托盘菜单，重构为以"实用工具"为主的结构。核心改动：完整实现麦克风设备枚举、选择、持久化、录音使用指定设备。

## 架构决策要点

### 设备标识：`DeviceTrait::id()` 而非 deprecated `name()`

cpal 0.17.3 提供了正确的 API：
- **`device.id()`** → `Result<DeviceId, DeviceIdError>` — 唯一标识，跨重启稳定
- **`host.device_by_id(&id)`** → `Option<Device>` — 反向查找
- **`device.description()`** → `Result<DeviceDescription, _>` — 用户可读名称（显示用）

`DeviceId` 实现 `Display + FromStr`，`to_string()` 直接存入 JSON 配置。

### URL 来源：编译时从 Cargo.toml 推导

```rust
const PKG_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
```

需在 `src-tauri/Cargo.toml` 补 `repository = "https://github.com/sinnohzeng/tingyuxuan-app"`。

### 无需 config version bump

新字段用 `#[serde(default)]`，与 `user_dictionary` 添加时模式一致。保持 `CURRENT_CONFIG_VERSION = 2`。

### 麦克风子菜单：惰性重建

cpal 无设备热插拔通知。策略：每次打开托盘菜单时重新枚举设备；录音时设备不存在则 fallback 到默认 + warn 日志。

### TrayIcon handle 必须保留

当前 `let _tray = ...build()?` 丢弃了 handle。需存入 managed state `TrayState` 以便重建菜单。

## 目标菜单结构

```
反馈意见                     → 打开 {CARGO_PKG_REPOSITORY}/issues
打开听语轩主页               → 显示主窗口
───────────
设置...                      → 显示主窗口 + 打开设置
选择麦克风                →  ┌ 系统默认 ✓        ┐
                              │ Studio Display Mic │
                              └ USB Headset        ┘
───────────
将词汇添加到词典             → 显示主窗口 + 导航到词典页
───────────
版本 {CARGO_PKG_VERSION}     → 灰色不可点击
检查更新                     → 打开 {CARGO_PKG_REPOSITORY}/releases/latest
───────────
退出听语轩
```

---

## 实现步骤（10 步）

### Step 1: 添加依赖 + Cargo.toml 元数据

| 文件 | 改动 |
|------|------|
| `src-tauri/Cargo.toml` | 添加 `tauri-plugin-opener = "2"`；`[package]` 加 `repository` 字段 |
| `src-tauri/capabilities/default.json` | permissions 添加 `"opener:default"` |
| `src-tauri/src/lib.rs` | 添加 `.plugin(tauri_plugin_opener::init())` |

### Step 2: 新建音频设备模块 `devices.rs`

**新建文件：** `crates/tingyuxuan-core/src/audio/devices.rs`（`#[cfg(feature = "audio")]` 门控）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,         // DeviceId.to_string() — 持久化标识
    pub name: String,       // DeviceDescription 可读名 — 显示用
    pub is_default: bool,
}

/// 枚举所有可用音频输入设备。Mock 模式返回模拟设备。
pub fn enumerate_input_devices() -> Result<Vec<AudioDeviceInfo>, AudioError>

/// 根据持久化的 DeviceId 字符串查找设备。
/// 找不到时 fallback 到 default_input_device() + warn 日志。
pub fn resolve_input_device(device_id: Option<&str>) -> Result<cpal::Device, AudioError>
```

`resolve_input_device` 放在 `devices.rs`——设备查找是设备管理的关注点，与录音解耦。

**修改文件：** `crates/tingyuxuan-core/src/audio/mod.rs` — 添加 `pub mod devices;`

### Step 3: 配置增加 AudioConfig

**修改文件：** `crates/tingyuxuan-core/src/config.rs`

```rust
/// 音频配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioConfig {
    /// 选中的麦克风设备 ID（DeviceId.to_string()）。None = 系统默认。
    #[serde(default)]
    pub input_device_id: Option<String>,
}
```

- `AppConfig` 添加 `#[serde(default)] pub audio: AudioConfig`
- `AppConfig::default()` 添加 `audio: AudioConfig::default()`
- **不** bump `CURRENT_CONFIG_VERSION`

新增测试：
- `test_config_backward_compat_no_audio`
- `test_config_with_audio_device`
- `test_default_audio_config_is_none`

### Step 4: 录音器支持指定设备

**修改文件：** `crates/tingyuxuan-core/src/audio/recorder.rs`

- `AudioRecorder` 新增字段 `device_id: Option<String>`
- `AudioRecorder::new()` 签名改为 `new(device_id: Option<&str>)`
- `start_real_stream()` 使用 `devices::resolve_input_device(self.device_id.as_deref())?`
- `probe_microphone()` 保持无参（只检测默认设备权限）

### Step 5: Recorder Actor 支持设备切换

**修改文件：** `src-tauri/src/recorder_actor.rs`

- `RecorderCommand` 新增 `SetDevice { device_id: Option<String>, reply: oneshot::Sender<()> }`
- `RecorderHandle::spawn()` 签名改为 `spawn(event_tx, device_id: Option<String>)`
- `RecorderHandle` 新增 `pub async fn set_device(&self, device_id: Option<String>)`
- `handle_command()` 处理 `SetDevice`：非录音时重建 recorder
- `drain_with_error()` 添加 `SetDevice` 分支

### Step 6: State 层 + TrayState

**修改文件：** `src-tauri/src/state.rs`

- 新增 `pub struct TrayState(pub Arc<Mutex<Option<tauri::tray::TrayIcon>>>);`
- `AppStates` 新增 `pub tray: TrayState`
- `AppStates::new()` 读取 `config.audio.input_device_id` 传给 `RecorderHandle::spawn()`

**修改文件：** `src-tauri/src/lib.rs` — `app.manage(states.tray)` 注册

### Step 7: 新增 Tauri 命令

**修改文件：** `src-tauri/src/commands.rs`

```rust
#[tauri::command]
pub async fn list_input_devices() -> Result<Vec<AudioDeviceInfo>, String>

#[tauri::command]
pub async fn set_input_device(
    device_id: Option<String>,
    config_state: State<'_, ConfigState>,
    recorder_state: State<'_, RecorderState>,
) -> Result<(), String>
```

这两个命令同时服务托盘菜单和未来的设置页面 AudioSection。

**修改文件：** `src-tauri/src/lib.rs` — 注册到 `invoke_handler`

### Step 8: 重写托盘菜单

**修改文件：** `src-tauri/src/tray.rs` — **完全重写**

函数拆分（≤30 行/函数）：

| 函数 | 职责 |
|------|------|
| `create_tray(app)` | 构建初始菜单 + TrayIconBuilder + 存入 TrayState |
| `rebuild_tray_menu(app)` | 从 TrayState 取 handle，重建菜单 + `set_menu()` |
| `build_menu(app)` | 构建完整 Menu 结构 |
| `build_mic_submenu(app)` | 动态麦克风子菜单（CheckMenuItem + 勾选） |
| `handle_menu_event(app, id)` | 事件路由 |
| `handle_mic_selection(app, id)` | 异步更新 config + recorder + 重建 |
| `open_url(app, url)` | `app.opener().open_url()` |
| `show_main_window(app)` | 已有，保留 |

关键设计：
- **URL 推导**：`env!("CARGO_PKG_REPOSITORY")` 编译时获取，不硬编码
- **版本号**：`env!("CARGO_PKG_VERSION")`
- **菜单 ID**：麦克风项 `"mic:{device_id}"` 前缀路由
- **惰性重建**：`on_tray_icon_event` 中每次 click 触发 `rebuild_tray_menu()`
- **TrayIconBuilder** 加 `.id("main")`

### Step 9: 前端事件监听更新

**修改文件：** `src/shared/components/MainLayout.tsx`

```diff
- useTauriEvent("open-history", useCallback(() => navigate("/main/history"), [navigate]));
+ useTauriEvent("open-dictionary", useCallback(() => navigate("/main/dictionary"), [navigate]));
```

### Step 10: 清理

- `tray.rs`：删除 `use crate::platform`（不再需要 shortcut_labels）
- 检查 `tauri-plugin-shell` 是否仍有其他用途，如无则移除并替换为 opener

---

## 关键文件清单

| 文件 | 操作 |
|------|------|
| `src-tauri/Cargo.toml` | 加 opener 依赖 + repository 元数据 |
| `src-tauri/capabilities/default.json` | 加 opener:default |
| `crates/tingyuxuan-core/src/audio/devices.rs` | **新建** — 设备枚举 + resolve |
| `crates/tingyuxuan-core/src/audio/mod.rs` | 加 devices 模块 |
| `crates/tingyuxuan-core/src/config.rs` | 加 AudioConfig（无 version bump） |
| `crates/tingyuxuan-core/src/audio/recorder.rs` | device_id 参数 + 用 resolve_input_device |
| `src-tauri/src/recorder_actor.rs` | SetDevice 命令 + spawn 参数 |
| `src-tauri/src/state.rs` | TrayState + device_id 传递 |
| `src-tauri/src/commands.rs` | 2 个新命令 |
| `src-tauri/src/tray.rs` | **完全重写** |
| `src-tauri/src/lib.rs` | 注册插件 + 命令 + TrayState |
| `src/shared/components/MainLayout.tsx` | open-history → open-dictionary |

## 文档同步（DDD + SSOT）

开发完成后同步更新：

| 文档 | 更新内容 |
|------|---------|
| `docs/architecture/adr/0009-audio-device-selection.md` | ADR：DeviceId vs name()、惰性重建、fallback 策略 |
| `docs/modules/audio.md` | 新增设备枚举章节 + AudioDeviceInfo + enumerate/resolve API |
| `docs/modules/config.md` | 新增 AudioConfig 章节 |
| `CLAUDE.md` | Tauri 命令清单加 `list_input_devices` / `set_input_device` |
| `docs/architecture/ui-design.md` | 托盘菜单新结构说明 |

## 测试计划

### 自动化测试

**devices.rs（新建）：**
- `test_enumerate_mock_mode` — TINGYUXUAN_MOCK_AUDIO=1 返回 mock 设备
- `test_mock_device_is_default` — mock 设备 is_default=true
- `test_audio_device_info_serialization` — JSON roundtrip
- `test_resolve_none_returns_default` — None → 默认设备（mock 模式）

**config.rs（追加）：**
- `test_config_backward_compat_no_audio` — 旧 JSON 无 audio 字段正常反序列化
- `test_config_with_audio_device` — audio.input_device_id roundtrip
- `test_default_audio_config_is_none` — AudioConfig::default().input_device_id == None

**recorder.rs（修改）：**
- 更新 `mock_recorder()` 传 `None`
- `test_new_with_device_id_mock` — `AudioRecorder::new(Some("x"))` mock 模式正常

### 手动验证
1. 托盘右键 → 所有菜单项按正确顺序显示
2. "反馈意见" → 浏览器打开 GitHub Issues
3. "检查更新" → 浏览器打开 GitHub Releases/latest
4. "打开听语轩主页" → 主窗口显示并聚焦
5. "设置..." → 主窗口 + 设置对话框
6. "将词汇添加到词典" → 主窗口 + 词典页
7. "选择麦克风" 子菜单 → 列出所有音频设备 + 当前选中有勾
8. 切换麦克风 → 勾选更新 + 配置持久化 + 重启后保持
9. 用非默认麦克风录音 → 正常工作
10. 拔掉选中的 USB 麦克风 → 下次录音 fallback 到默认设备
11. "版本 X.Y.Z" → 灰色不可点击
12. "退出听语轩" → 进程退出

### 构建验证
```bash
cargo check -p tingyuxuan-core --no-default-features   # 核心检查（无音频）
cargo test -p tingyuxuan-core                           # Rust 测试
npx tsc --noEmit                                        # TypeScript 类型检查
npm test                                                # 前端测试
```
