import type { ProviderPreset } from "./types";

export const PROVIDER_PRESETS: Record<string, ProviderPreset> = {
  dashscope: {
    name: "阿里云 DashScope",
    llm_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    llm_models: ["qwen-turbo", "qwen-plus", "qwen-max"],
    stt_provider: "dashscope_asr",
    stt_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    stt_model: "qwen2-audio-instruct",
  },
  volcengine: {
    name: "火山引擎 (豆包)",
    llm_base_url: "https://ark.cn-beijing.volces.com/api/v3",
    llm_models: ["doubao-1-5-pro-256k", "doubao-1-5-lite-32k"],
    stt_provider: "whisper",
    stt_base_url: null,
    stt_model: null,
  },
  openai: {
    name: "OpenAI",
    llm_base_url: "https://api.openai.com/v1",
    llm_models: ["gpt-4o", "gpt-4o-mini"],
    stt_provider: "whisper",
    stt_base_url: "https://api.openai.com/v1",
    stt_model: "whisper-1",
  },
};
