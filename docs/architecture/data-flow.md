# 管线数据流

## 核心处理流程

```
用户按下快捷键 (RAlt)
        │
        ▼
[Global Shortcut Handler] ──emit──> "shortcut-action"
        │
        ▼
[Frontend: FloatingBar] ──invoke──> start_recording(mode)
        │
        ▼
[Tauri Command] ──────────────────────────────────────────┐
  1. 校验 Pipeline 可用性（API Key 已配置？）              │
  2. 检测上下文（选中文本？→ 切换 Edit 模式）              │
  3. 获取活动窗口名（上下文感知提示词）                    │
  4. 创建历史记录（status: recording）                    │
  5. 启动录音器 Actor（发送 Start 命令）                   │
  6. emit RecordingStarted 事件                          │
        │                                                │
        ▼                                                │
[Recorder Actor: 专用 OS 线程]                            │
  - cpal 音频流回调                                       │
  - 16kHz mono WAV 写入                                   │
  - RMS 计算 → VolumeUpdate 事件 → 前端波形可视化          │
  - 每 500ms flush 防崩溃丢失                             │
        │                                                │
        ▼ (用户按完成/再按快捷键)                          │
[Frontend] ──invoke──> stop_recording()                   │
        │                                                │
        ▼                                                │
[Tauri Command]                                          │
  1. 停止录音器，获取 duration_ms                         │
  2. 检查网络状态                                        │
  3a. 在线 → 异步管线处理（见下）                         │
  3b. 离线 → 加入 OfflineQueue（status: queued）          │
        │                                                │
        ▼ (在线路径)                                      │
[Pipeline::process_audio] (tokio::spawn 后台任务)          │
  ┌──────────────────────────────────────────┐           │
  │ Stage 1: STT                             │           │
  │  - emit TranscriptionStarted             │           │
  │  - 调用 stt.transcribe() (with retry)    │           │
  │  - emit TranscriptionComplete            │           │
  ├──────────────────────────────────────────┤           │
  │ Stage 2: LLM                             │           │
  │  - emit ProcessingStarted                │           │
  │  - 构建 LLMInput (mode + context + dict) │           │
  │  - 调用 llm.process() (with retry)       │           │
  │  - emit ProcessingComplete               │           │
  └──────────────────────────────────────────┘           │
        │                                                │
        ▼                                                │
  Dictate/Translate/Edit:                                │
    text_injector::inject_text(processed_text)            │
    → 50ms delay → xdotool type / clipboard paste         │
  AI Assistant:                                          │
    不自动注入，仅 emit ProcessingComplete                │
    → 前端 ResultPanel 展示结果                           │
└─────────────────────────────────────────────────────────┘
```

## 事件桥接架构

> 详见 [ADR-0004: 事件桥接 Push 模式](adr/0004-event-bridge-push-model.md)

```
Rust Core                    Tauri Backend                 React Frontend
─────────                    ─────────────                 ──────────────
broadcast::Sender     ──>    event_rx.recv()        ──>   app.emit("pipeline-event")
<PipelineEvent>              │                             │
                             ├─ 窗口可见性管理              ├─ listen("pipeline-event")
                             │  RecordingStarted → show    │  更新 appStore 状态
                             │  Error → show               │  驱动 FloatingBar 渲染
                             │  ProcessingComplete → (等)   │
                             │                             │
                             └─ 网络状态监控               └─ 状态映射:
                                NetworkStatusChanged          idle → 隐藏
                                → 更新 AtomicBool             recording → 波形
                                → online: drain queue          processing → 旋转
                                                              done → ✓ 完成
                                                              error → 错误面板
```

## 离线队列数据流

```
stop_recording() 检测到 offline
        │
        ▼
OfflineQueue.enqueue(QueuedRecording)
  - session_id, audio_path, mode, context
  - History status → "queued"
        │
        │ ... 网络恢复 ...
        │
NetworkMonitor (30s HEAD 探测) → NetworkStatusChanged { online: true }
        │
        ▼
Event Bridge 收到 online = true
        │
        ▼
queue.drain() → Vec<QueuedRecording>
        │
        ▼
对每个 item: tokio::spawn Pipeline::process_audio()
```

## 窗口管理

| 窗口 | 标签 | 尺寸 | 可见性 |
|------|------|------|--------|
| FloatingBar | `floating-bar` | 420×64 (AI 助手扩展到 420×360) | 按需 show/hide |
| Settings | `settings` | 640×520 | 按需 show |

FloatingBar 可见性由事件桥接控制：
- `RecordingStarted` → show + focus
- `Error` → show + focus
- `ProcessingComplete` + done 状态 1.5s → 前端调用 `window.hide()`
- AI Assistant done → 不自动隐藏（等待用户操作 ResultPanel）
