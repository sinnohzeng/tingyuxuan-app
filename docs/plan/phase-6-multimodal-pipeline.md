# Phase 6: 多模态一步管线重构

**状态**: Done
**日期**: 2026-03-01

## 背景

当前项目采用两步串行管线：麦克风 → DashScope STT → LLM 润色 → 文本注入。
竞品 Typeless 采用单步架构：音频 + 屏幕上下文 → 多模态大模型一步完成。

本次重构完全替换为单步多模态管线，删除独立 STT 模块。

```
变更前：麦克风 → PCM → DashScope STT → 原始文字 → LLM → 注入  (2次 API)
变更后：麦克风 → PCM → 编码 → base64 + 上下文 → 多模态 LLM → 注入  (1次 API)
```

## 目标提供商

- **阿里云 DashScope Qwen-Omni**（推荐）：`qwen3-omni-flash`，base_url `https://dashscope.aliyuncs.com/compatible-mode/v1`
- **OpenAI**（备选）：`gpt-4o-audio-preview`

## 关键技术约束

- OpenAI `input_audio` 仅支持 wav 和 mp3
- 初始实现 WAV（零依赖，PCM + 44 字节头）
- Qwen-Omni **强制 `stream=true`**，必须用 SSE 流式解析
- 只需文本输出：`modalities: ["text"]`

## 实现阶段

详见 ADR-0008 和各 Phase 实现代码。

| Phase | 内容 | 状态 |
|-------|------|------|
| 0 | 文档持久化 + ADR | Done |
| 1 | 音频编码抽象 + 缓冲区 | Done |
| 2 | 多模态 LLM Provider | Done |
| 3 | 管线重构 + 配置简化 + 错误清理 | Done |
| 4 | Tauri 命令层适配 | Done |
| 5 | 前端适配 | Done |
| 6 | 文档同步 + 验证 | Done |
