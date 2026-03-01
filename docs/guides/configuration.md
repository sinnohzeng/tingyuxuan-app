# 配置指南

## 打开设置

在系统托盘中点击听语轩图标，选择「设置」即可打开配置窗口。

---

## LLM（多模态大语言模型）配置

听语轩使用多模态大语言模型一步完成语音识别和文本处理，只需配置一个 API Key。

### 支持的 Provider

| Provider | 说明 | 默认 Base URL | 推荐模型 |
|----------|------|--------------|----------|
| **阿里云 DashScope**（推荐） | 灵积 Qwen3-Omni 系列 | `https://dashscope.aliyuncs.com/compatible-mode/v1` | qwen3-omni-flash |
| **OpenAI** | GPT-4o Audio | `https://api.openai.com/v1` | gpt-4o-audio-preview |
| **自定义** | 任何兼容 OpenAI Chat Completions API 且支持音频输入的服务 | 用户自定义 | — |

> **重要：** 所选模型必须支持音频输入（multimodal audio）。纯文本模型（如 gpt-4o-mini、qwen-turbo）无法正常工作。

### 配置步骤

1. 选择 LLM Provider（推荐阿里云 DashScope）
2. 输入 API Key（安全存储在操作系统 keyring 中）
3. 可选：修改 Base URL（自建服务时）
4. 选择支持音频输入的模型（默认：qwen3-omni-flash）
5. 点击「测试连接」验证配置

### API Key 安全性

- API Key 存储在操作系统的 keyring 中（Linux: GNOME Keyring / KWallet）
- 配置文件中仅保存引用标记 `@keyref:llm_api_key`，不保存明文
- 如果 keyring 不可用，会降级为配置文件明文存储

---

## 语言配置

| 设置项 | 说明 | 默认值 |
|-------|------|-------|
| 主语言 | 提示 LLM 的主要语言 | `zh` （中文） |
| 翻译目标语言 | 翻译模式的目标语言 | `en` （英语） |
| 语言变体 | 区域变体（可选） | 无 |

---

## 用户词典

用户词典可以帮助 LLM 更准确地处理专有名词和术语：

1. 在设置中找到「用户词典」区域
2. 输入词汇后点击添加
3. 每个词汇最长 100 字节

LLM 处理时会参考词典中的词汇，提高专有名词的识别和使用准确度。

---

## 通用设置

| 设置项 | 说明 | 默认值 |
|-------|------|-------|
| 开机自启 | 系统启动时自动运行 | 关 |
| 声音反馈 | 录音开始/结束时播放提示音 | 开 |
| 悬浮栏位置 | 底部居中 / 跟随光标 / 固定位置 | 底部居中 |

---

## 配置文件位置

配置文件存储在标准目录中：

```
~/.config/tingyuxuan/TingYuXuan/config.json
```

> **注意**：直接编辑配置文件时，确保 JSON 格式正确。建议通过设置界面修改。
