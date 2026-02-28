import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import ApiConfig from "./ApiConfig";
import type { AppConfig } from "../../lib/types";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));

const defaultConfig: AppConfig = {
  general: {
    auto_launch: false,
    sound_feedback: true,
    floating_bar_position: "bottom_center",
  },
  shortcuts: {
    dictate: "alt_right",
    translate: "shift+alt_right",
    ai_assistant: "alt+space",
    cancel: "escape",
  },
  language: {
    primary: "zh-CN",
    translation_target: "en",
    variant: null,
  },
  stt: {
    provider: "dashscope_streaming",
    api_key_ref: "stt",
    base_url: null,
    model: null,
  },
  llm: {
    provider: "openai",
    api_key_ref: "llm",
    base_url: null,
    model: "gpt-4o-mini",
  },
  cache: {
    history_retention_days: 30,
  },
  user_dictionary: [],
};

describe("ApiConfig", () => {
  it("renders section headers", () => {
    render(<ApiConfig config={defaultConfig} onUpdate={vi.fn()} />);

    expect(screen.getByText("快速配置")).toBeInTheDocument();
    expect(screen.getByText("语音识别 (STT)")).toBeInTheDocument();
    expect(screen.getByText("大语言模型 (LLM)")).toBeInTheDocument();
  });

  it("renders STT configuration section with provider select", () => {
    render(<ApiConfig config={defaultConfig} onUpdate={vi.fn()} />);

    // STT provider option should be present
    expect(screen.getByText("阿里云 DashScope（流式）")).toBeInTheDocument();
  });

  it("renders LLM configuration section with provider select", () => {
    render(<ApiConfig config={defaultConfig} onUpdate={vi.fn()} />);

    // LLM provider options should be present (use selector to disambiguate from preset buttons)
    expect(screen.getByText("OpenAI", { selector: "option" })).toBeInTheDocument();
    expect(screen.getByText("阿里云 DashScope", { selector: "option" })).toBeInTheDocument();
    expect(screen.getByText("火山引擎 (豆包)", { selector: "option" })).toBeInTheDocument();
  });

  it("renders provider preset buttons", () => {
    render(<ApiConfig config={defaultConfig} onUpdate={vi.fn()} />);

    // Preset button from PROVIDER_PRESETS (MVP: only DashScope)
    expect(screen.getByText("阿里云 DashScope", { selector: "button" })).toBeInTheDocument();
  });

  it("renders API key inputs and save buttons", () => {
    render(<ApiConfig config={defaultConfig} onUpdate={vi.fn()} />);

    const saveButtons = screen.getAllByText("保存");
    expect(saveButtons).toHaveLength(2); // One for STT, one for LLM
  });

  it("renders test connection buttons", () => {
    render(<ApiConfig config={defaultConfig} onUpdate={vi.fn()} />);

    const testButtons = screen.getAllByText("测试连接");
    expect(testButtons).toHaveLength(2); // One for STT, one for LLM
  });
});
