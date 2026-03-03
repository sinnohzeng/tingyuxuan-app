# ADR-0009: 音频设备选择 — DeviceId 标识 + 惰性重建策略

**状态**: Accepted
**日期**: 2026-03-03

## 背景

托盘菜单重构需要支持用户选择麦克风输入设备。cpal 0.17.3 提供了多种设备标识方式，且缺乏设备热插拔通知 API，需要确定：

1. 设备标识方案：如何唯一标识并持久化用户选择的设备？
2. 设备枚举时机：何时刷新可用设备列表？
3. 设备不可用时的容错策略。

## 决策

### 1. 使用 `DeviceTrait::id()` 作为持久化标识

cpal 0.17.3 提供三种设备标识 API：

| API | 返回类型 | 特点 |
|-----|---------|------|
| `device.id()` | `Result<DeviceId, _>` | 唯一标识，跨重启稳定，支持 `Display + FromStr` |
| `device.description()` | `Result<DeviceDescription, _>` | 人类可读名称，用于 UI 显示 |
| `device.name()` | `Result<String, _>` | **已 deprecated** |

选择 `device.id()` 的 `to_string()` 作为配置文件中的持久化标识，`device.description()` 作为 UI 显示名称。通过 `host.device_by_id()` 反向查找设备。

### 2. 惰性重建策略

cpal 无设备热插拔通知。采用**惰性重建**：

- 每次用户右键打开托盘菜单时，重新枚举所有音频输入设备
- 构建 `CheckMenuItem` 子菜单，当前选中设备显示勾选标记
- 不使用定时器轮询，避免不必要的系统调用开销

### 3. Fallback 到默认设备

当持久化的 `device_id` 对应设备不可用时（如 USB 麦克风已拔出）：

- `resolve_input_device()` fallback 到系统默认输入设备
- 输出 `warn` 级别日志（包含原 device_id 和 fallback 信息）
- 不自动清除配置中的 `input_device_id`，避免用户重新插入设备后需重新选择

## 后果

### 正面

- **跨重启稳定**：`DeviceId` 在设备不变的情况下跨应用重启保持一致
- **零轮询开销**：惰性重建只在用户操作时触发
- **优雅降级**：设备消失时自动 fallback 到默认设备，不中断使用
- **配置向后兼容**：`AudioConfig` 使用 `#[serde(default)]`，旧配置文件无需迁移

### 负面

- **菜单刷新延迟**：设备变更不会实时反映在已打开的菜单中（需关闭后重新打开）
- **跨系统不可移植**：`DeviceId` 格式因操作系统而异，配置文件在不同 OS 间不通用
- **fallback 静默**：用户可能不知道正在使用的是 fallback 设备而非选中设备

## 备选方案

### A. 使用 `device.name()` 作为标识

`name()` 已被 cpal 标记为 deprecated，且在某些平台上名称不唯一（多个同型号设备可能同名）。

### B. 定时轮询设备列表

每 N 秒枚举一次设备，实时更新菜单。开销较大且大部分时间无变化，不值得。

### C. 配置中同时存储 id 和 name

可以在设备匹配失败时按 name 模糊查找。增加了复杂度，且 name 匹配不可靠。YAGNI。
