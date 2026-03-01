# ADR-0008: 单步多模态管线替代两步 STT+LLM 管线

**状态**: Accepted
**日期**: 2026-03-01

## 背景

v0.6 采用两步串行管线：流式 STT（DashScope Paraformer WebSocket）→ LLM 润色。
存在两个问题：

1. **延迟**：两次网络往返（STT WebSocket + LLM HTTP），总延迟 2-5 秒
2. **信息损失**：STT 丢失语音中的语气、停顿、重音等韵律信息，LLM 只能基于文本猜测意图

Qwen-Omni 系列（2025 年发布）支持音频直接输入，一步完成识别和润色。

## 决策

完全替换为单步多模态管线：

- 录音时在内存中累积 PCM 缓冲区（不再通过 channel 流式转发）
- 录音结束后编码为 WAV（44 字节 RIFF 头 + raw PCM16，零依赖）
- base64 编码后通过 OpenAI 兼容的 Chat Completions API 发送到多模态 LLM
- SSE 流式解析响应（Qwen-Omni 强制 `stream=true`）
- 删除整个 `stt/` 模块和所有 STT 相关配置

音频编码抽象为 `AudioFormat` 枚举 + `AudioBuffer::encode()` trait method，
新格式只需加枚举值 + match 分支。

## 后果

**正面**：
- 单次 API 调用，减少延迟
- LLM 直接处理音频，保留韵律信息，提高理解准确性
- 代码大幅简化：删除 ~1200 行 STT 代码，移除 WebSocket 依赖
- 配置简化：只需一个 API key + 模型选择
- 统一 SSE 流式处理，简化错误处理

**负面**：
- 依赖模型的音频能力（非所有 LLM 支持音频输入）
- 录音期间无实时转写反馈（单步架构固有限制）
- 长录音的音频编码和上传可能增加处理时间

## 备选方案

1. **保留 STT + 升级到更好的 STT 模型**：不解决信息损失问题
2. **STT + 多模态 LLM 并行**：复杂度过高，维护成本大
3. **自建中继服务器支持 Opus 编码**：增加运维成本，不适合客户端直连架构
