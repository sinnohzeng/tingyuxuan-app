# ADR-0006: 平台抽象层

**状态**: Accepted
**日期**: 2026-02 (Phase 5)

## 背景

听语轩 v0.1.0 仅支持 Linux 桌面端。PRD 明确定义 Windows 为 P0 优先级平台，Android 为 P2 优先级。
当前所有平台特定代码（`text_injector.rs`、`context.rs`）仅有 Linux 实现，需要建立一个可扩展的平台抽象层。

## 决策

使用 **编译时类型别名 + trait** 实现零开销平台抽象。

### 核心设计

1. **Trait 定义**：`TextInjector` 和 `ContextDetector` 两个 trait 定义平台需要实现的接口
2. **编译时分发**：`#[cfg(target_os = "...")]` 类型别名代替 `Box<dyn Trait>` 动态派发
   ```rust
   #[cfg(target_os = "linux")]
   pub type PlatformInjector = linux::LinuxTextInjector;
   #[cfg(target_os = "windows")]
   pub type PlatformInjector = windows::WindowsTextInjector;
   ```
3. **Managed State**：`InjectorState` 和 `DetectorState` 在 `AppStates::new()` 中创建一次，
   通过 Tauri Managed State 注入（与现有 8 个 State 模式一致，总计 10 个）
4. **结构化错误**：`PlatformError`（thiserror）代替 `Result<_, String>`
5. **剪贴板 DRY**：clipboard save/write/paste/restore 抽为 primitive 函数 + 组合函数，
   避免各平台重复实现

### 文件结构

```
src-tauri/src/platform/
├── mod.rs        # trait 定义 + sanitize_for_typing + 类型别名
├── error.rs      # PlatformError (thiserror)
├── linux.rs      # LinuxTextInjector + LinuxContextDetector
└── windows.rs    # (Step 1-2 添加)
```

## 后果

**正面**：
- 零运行时开销：无堆分配、无 vtable 查找
- 编译期保证：错误的平台代码在编译时即被发现
- 统一错误类型：`PlatformError` 可被 pattern match，便于不同错误分别处理
- 可扩展：添加新平台只需新增 `mod` + 类型别名

**负面**：
- 需要 `#[cfg]` 条件编译，某些 IDE 可能只显示当前平台的代码
- 无法在运行时动态切换平台实现（但这不是实际需求）

## 备选方案

| 方案 | 未选择原因 |
|------|-----------|
| `Box<dyn Trait>` 动态派发 | 平台编译时已知，堆分配 + vtable 是不必要的开销 |
| 每次调用创建新实例 | 反复创建浪费资源，不如创建一次作为 Managed State |
| `Result<_, String>` | 非结构化错误，无法 pattern match，不符合代码品味 |
