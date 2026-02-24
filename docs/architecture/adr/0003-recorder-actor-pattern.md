# ADR-0003: 录音器 Actor 模式

**状态**: Accepted
**日期**: 2025-02 (Phase 2)

## 背景

cpal 音频流的回调函数在专用音频线程上执行，受实时约束——不能在回调中执行任何可能阻塞的操作（分配内存、获取 Mutex、I/O）。同时，录音器需要响应来自 Tauri 命令的控制指令（start、stop、cancel）和向前端推送音量数据。

直接在 Tauri 命令中操作 `AudioRecorder` 的 `Arc<Mutex<...>>` 存在风险：
- 命令线程获取 Mutex 可能与音频回调竞争
- tokio 异步上下文中的 `.blocking_lock()` 可能阻塞 runtime

## 决策

采用 **Actor 模式**：录音器运行在专用 OS 线程上，通过 `mpsc::channel` 接收命令，通过 `broadcast::Sender` 推送事件。

```
Tauri Commands ──(mpsc::Sender<RecorderCommand>)──> [OS Thread: RecorderActor]
                                                            │
                                                            ├── Start(session_id, mode, cache_dir)
                                                            ├── Stop → returns audio_path + duration
                                                            ├── Cancel → deletes WAV
                                                            └── IsRecording → returns bool
                                                            │
                                                    (broadcast::Sender<PipelineEvent>)
                                                            │
                                                            ▼
                                                    VolumeUpdate { levels }
```

`RecorderHandle` 是面向调用者的句柄，提供异步 API：
- `spawn(event_tx)` — 创建 Actor 线程
- `start(session_id, mode, cache_dir)` — 发送 Start 命令
- `stop()` — 发送 Stop 命令，通过 oneshot channel 接收结果
- `cancel()` — 发送 Cancel 命令
- `is_recording()` — 查询状态

## 后果

**正面**：
- 音频回调和控制命令在同一线程上串行执行，无竞态条件
- Tauri 命令只需 `await` 异步结果，不直接操作音频 API
- 音量更新通过 EventBus push 到前端，无轮询
- 崩溃隔离：录音器线程 panic 不影响主进程（可通过 handle 检测）

**负面**：
- 增加了 channel 通信开销（微秒级，可忽略）
- 调试时需要跨线程追踪
- oneshot channel 在 Actor 线程退出时会产生 RecvError

## 备选方案

| 方案 | 未选择原因 |
|------|-----------|
| `Arc<Mutex<AudioRecorder>>` | 音频回调中无法安全获取 Mutex |
| `Arc<RwLock>` + atomic flags | 复杂度高，仍有潜在竞态 |
| tokio::task::spawn_blocking | cpal 需要持续运行的 OS 线程，不适合 spawn_blocking |
