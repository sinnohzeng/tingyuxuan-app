# Qwen3-ASR-Flash 传输与压缩决策（2026-03-03）

## 背景

目标模型切换到 `qwen3-asr-flash`（中国内地部署，北京地域）后，需要确定两件关键策略：

1. 客户端录音文件如何压缩（WAV / MP3 / OPUS）
2. 请求体使用音频 URL 还是 Base64（Data URL）

本文件是该问题的 SSOT 决策记录，供后续代码实现直接落地。

## 约束与事实（官方文档）

1. `qwen3-asr-flash` 适用于短音频，同步/流式皆可；短音频限制为**文件 <=10MB 且 <=5 分钟**。
2. OpenAI 兼容接口与 DashScope 接口都支持北京地域接入点。
3. 输入可用两种方式：
   - 公网可访问 URL
   - Base64 Data URL（`data:<mime>;base64,<data>`）
4. 文档明确提示 Base64 会放大体积，应保证编码后仍满足 10MB 限制。
5. 支持格式包含 `mp3` / `opus` / `wav` 等。

## 计算与工程判断

### 1) 体积预算（5 分钟）

1. WAV（16kHz/16-bit/mono）约 `32KB/s`，5 分钟约 `9.6MB`，Base64 后约 `12.8MB`（超限）。
2. MP3（24kbps）约 `3KB/s`，5 分钟约 `0.9MB`，Base64 后约 `1.2MB`（安全）。
3. OPUS（16~24kbps）通常更小，但桌面端纯 Rust/免外部工具链编码稳定性与集成复杂度高于 MP3。

### 2) URL vs Base64

1. Base64 直传：
   - 优点：链路最短、无需 OSS 上传、端到端更快、实现简单。
   - 缺点：有 33% 膨胀，受 10MB 强约束。
2. URL：
   - 优点：规避 Base64 膨胀；长音频/大文件可扩展到 Filetrans（URL 必选）。
   - 缺点：需要额外上传与对象生命周期管理；增加时延与隐私暴露面。

## 最终决策

### 决策 A：压缩格式

1. **MVP 默认使用 MP3（24kbps，单声道）**。
2. MP3 编码失败时回退 WAV。
3. OPUS 作为后续优化项（当编码栈与发布链路稳定后再切换默认）。

### 决策 B：传输方式

1. **MVP 默认使用 Base64 Data URL 直传**（低复杂度、低时延）。
2. **当前版本不处理 >5 分钟录音**，不引入 `filetrans` 分流逻辑。
3. URL 上传通道仅作为后续扩展预案，不进入本阶段实现。

## 落地策略（可直接编码）

1. 客户端编码后统一走 Base64 Data URL（`data:audio/mpeg;base64,...`）。
2. 录音时长由客户端/录音器双重限制在 `<= 5 分钟`。
3. 北京地域固定：
   - OpenAI 兼容：`https://dashscope.aliyuncs.com/compatible-mode/v1`
   - DashScope：`https://dashscope.aliyuncs.com/api/v1`

## 对 Typeless 路线的映射

1. Typeless 的核心体验是“批处理上传 + 上下文 + 一次性返回”，与 Base64/URL 两种输入方式都兼容。
2. 在我们的工程现状下，**MP3 + Base64**更接近“最短路径上线”，可先稳定体验再引入 URL 分流。

## 参考

1. https://help.aliyun.com/zh/model-studio/qwen-speech-recognition
2. https://help.aliyun.com/zh/model-studio/qwen-asr-api-reference
