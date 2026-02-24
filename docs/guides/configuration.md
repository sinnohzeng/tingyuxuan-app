# 配置指南

## 打开设置

在系统托盘中点击听语轩图标，选择「设置」即可打开配置窗口。

---

## STT（语音识别）配置

### 支持的 Provider

| Provider | 说明 | 默认 Base URL |
|----------|------|--------------|
| **Whisper** | OpenAI Whisper API（兼容格式） | `https://api.openai.com/v1` |
| **DashScope ASR** | 阿里云灵积语音识别 | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| **自定义** | 任何兼容 Whisper API 格式的服务 | 用户自定义 |

### 配置步骤

1. 选择 STT Provider
2. 输入 API Key（安全存储在操作系统 keyring 中）
3. 可选：修改 Base URL（自建服务时）
4. 可选：指定模型名称
5. 点击「测试连接」验证配置

### API Key 安全性

- API Key 存储在操作系统的 keyring 中（Linux: GNOME Keyring / KWallet）
- 配置文件中仅保存引用标记 `@keyref:stt`，不保存明文
- 如果 keyring 不可用，会降级为配置文件明文存储

---

## LLM（大语言模型）配置

### 支持的 Provider

| Provider | 说明 | 默认 Base URL |
|----------|------|--------------|
| **OpenAI** | OpenAI ChatGPT API | `https://api.openai.com/v1` |
| **DashScope** | 阿里云灵积 | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| **火山引擎** | 字节跳动火山引擎 | `https://ark.cn-beijing.volces.com/api/v3` |
| **自定义** | 任何兼容 OpenAI Chat Completions API 的服务 | 用户自定义 |

### 配置步骤

1. 选择 LLM Provider
2. 输入 API Key
3. 可选：修改 Base URL
4. 指定模型名称（如 `gpt-4o-mini`、`qwen-turbo` 等）
5. 点击「测试连接」验证配置

---

## 语言配置

| 设置项 | 说明 | 默认值 |
|-------|------|-------|
| 主语言 | STT 识别的主要语言 | `zh` （中文） |
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
