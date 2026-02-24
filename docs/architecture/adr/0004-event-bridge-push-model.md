# ADR-0004: 事件桥接 Push 模式

**状态**: Accepted
**日期**: 2025-02 (Phase 2)

## 背景

管线处理（STT → LLM）是异步后台任务，前端需要实时感知处理进度以更新浮动条 UI（波形、"处理中..."、"完成"、错误面板）。需要一个机制将后端状态变化传递到前端。

## 决策

采用 **broadcast → app.emit Push 模式**：

1. Rust Core 定义 `PipelineEvent` 枚举（`#[derive(Serialize)]` + `#[serde(tag = "type")]`）
2. Pipeline、Recorder、NetworkMonitor 通过 `broadcast::Sender<PipelineEvent>` 发送事件
3. Tauri Backend 的事件桥接任务（`tokio::spawn`）订阅 broadcast channel
4. 收到事件后：
   - 管理窗口可见性（show/hide FloatingBar）
   - 处理网络恢复（drain 离线队列）
   - 通过 `app.emit("pipeline-event", &event)` 转发到前端
5. 前端通过 `listen("pipeline-event")` 接收，更新 Zustand store

```
broadcast::Sender ──> event_rx.recv() ──> app.emit() ──> listen()
                      (Tauri Backend)                    (React)
```

## 后果

**正面**：
- 实时更新：事件从产生到前端 UI 更新 < 5ms
- 解耦：Core 库不依赖 Tauri，通过 broadcast channel 抽象
- 多消费者：event bridge 和 future 的日志/分析可以同时订阅
- 类型安全：`PipelineEvent` 的 `#[serde(tag = "type")]` 产生 `{ type: "TranscriptionComplete", raw_text: "..." }` 格式

**负面**：
- broadcast channel 在无订阅者时静默丢弃事件（设计上可接受）
- 前端需要 `listen` 的 cleanup（`unlisten` in useEffect return）
- 事件是单向的（后端 → 前端），前端到后端仍通过 `invoke`

## 备选方案

| 方案 | 未选择原因 |
|------|-----------|
| 前端轮询 (`setInterval` + `invoke`) | 高延迟、CPU 浪费、无法实时波形 |
| WebSocket | Tauri 已提供 IPC，无需额外 WS 服务器 |
| Tauri 插件事件 | 过于低层，需要手动序列化 |
