import type { ProviderPreset } from "./types";

export const PROVIDER_PRESETS: Record<string, ProviderPreset> = {
  dashscope: {
    name: "阿里云 DashScope",
    llm_provider: "dashscope",
    llm_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    llm_models: ["qwen-turbo", "qwen-plus", "qwen-max"],
    stt_provider: "dashscope_streaming",
    stt_base_url: null,
    stt_model: "paraformer-realtime-v2",
  },
};
