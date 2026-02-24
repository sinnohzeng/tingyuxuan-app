# ADR-0002: 分离 Managed State 架构

**状态**: Accepted
**日期**: 2025-02 (Phase 2)

## 背景

Phase 1 使用单一 `AppState` 结构体（`Arc<Mutex<AppState>>`）管理所有应用状态。随着 Phase 2 功能增加（录音、管线、配置、历史、队列、网络监控），单一 Mutex 产生严重的锁竞争：
- 录音器每 30ms 更新音量数据，需要 lock
- 配置读取是高频操作，被录音 lock 阻塞
- 管线重建（配置变更时）需要长时间持有 lock

## 决策

将单一 `AppState` 拆分为 **8 个独立 Managed State**，每个使用最合适的同步原语：

```rust
ConfigState(Arc<RwLock<AppConfig>>)         // 读多写少 → RwLock
HistoryState(Arc<Mutex<HistoryManager>>)    // SQLite 独占 → Mutex
PipelineState(Arc<RwLock<Option<Arc<Pipeline>>>>)  // 可重建 → RwLock + Option
EventBus(broadcast::Sender<PipelineEvent>)  // 无锁广播
SessionState(Arc<Mutex<Option<ActiveSession>>>)    // 短时持有
RecorderState(RecorderHandle)               // Actor 句柄（无锁）
QueueState(Arc<Mutex<OfflineQueue>>)        // 低频操作
NetworkState(Arc<AtomicBool>)               // 原子操作（无锁）
```

每个 State 通过 `app.manage()` 独立注册到 Tauri，命令函数按需声明依赖。

## 后果

**正面**：
- 消除锁竞争：录音音量更新不阻塞配置读取
- 精确控制：RwLock 用于读多写少（Config、Pipeline），Mutex 用于需要独占的（History、Session）
- AtomicBool 用于网络状态，完全无锁
- 每个命令只请求需要的 State，提升代码可读性

**负面**：
- 命令函数签名变长（多个 `State<'_, T>` 参数）
- 多个 State 间的一致性需要开发者自行保证（无事务）
- 状态初始化逻辑集中在 `AppStates::new()` 中

## 备选方案

| 方案 | 未选择原因 |
|------|-----------|
| 单一 `Arc<Mutex<AppState>>` | 锁竞争严重，音量更新 30ms/次会阻塞其他操作 |
| 分组 State（3-4 组） | 粒度不够细，仍然存在不必要的锁等待 |
| Actor 全 State | 过度设计，大部分 State 不需要 Actor 模式 |
