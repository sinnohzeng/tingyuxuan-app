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
  - 16kHz mono PCM 累积到 AudioBuffer                     │
  - RMS 计算 → VolumeUpdate 事件 → 前端波形可视化          │
        │                                                │
        ▼ (用户按完成/再按快捷键)                          │
[Frontend] ──invoke──> stop_recording()                   │
        │                                                │
        ▼                                                │
[Tauri Command]                                          │
  1. 停止录音器，获取 AudioBuffer                         │
  2. 检查 AudioBuffer 是否为空                            │
  3. 异步管线处理（见下）                                  │
        │                                                │
        ▼                                                │
[Pipeline::process_audio] (tokio::spawn 后台任务)          │
  ┌──────────────────────────────────────────┐           │
  │ 单步多模态处理:                           │           │
  │  1. AudioBuffer.encode(Wav) → EncodedAudio│           │
  │  2. EncodedAudio.to_base64() → base64     │           │
  │  3. 构建 system prompt (模式+上下文+词典)  │           │
  │  4. emit ProcessingStarted               │           │
  │  5. POST /chat/completions               │           │
  │     body: { messages: [system, audio] }   │           │
  │  6. SSE 流式解析 delta.content            │           │
  │  7. emit ProcessingComplete              │           │
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
                                                              processing → 旋转
                                                              done → 完成
                                                              error → 错误面板
```

## 窗口管理

| 窗口 | 标签 | 尺寸 | 可见性 |
|------|------|------|--------|
| FloatingBar | `floating-bar` | 420x64 (AI 助手扩展到 420x360) | 按需 show/hide |
| Settings | `settings` | 640x520 | 按需 show |

FloatingBar 可见性由事件桥接控制：
- `RecordingStarted` → show + focus
- `Error` → show + focus
- `ProcessingComplete` + done 状态 1.5s → 前端调用 `window.hide()`
- AI Assistant done → 不自动隐藏（等待用户操作 ResultPanel）
