import type { ProviderPreset } from "./types";

export const PROVIDER_PRESETS: Record<string, ProviderPreset> = {
  dashscope: {
    name: "阿里云 Qwen3-Omni Flash（推荐）",
    provider: "dashscope",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    models: ["qwen3-omni-flash", "qwen-omni-turbo"],
  },
  openai: {
    name: "OpenAI GPT-4o Audio",
    provider: "openai",
    base_url: "https://api.openai.com/v1",
    models: ["gpt-4o-audio-preview"],
  },
};
