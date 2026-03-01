import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { renderWithProviders, resetStores } from "../../test-utils";
import { useUIStore } from "../../shared/stores/uiStore";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import SettingsDialog from "./SettingsDialog";

const DEFAULT_CONFIG = {
  llm: { provider: "dashscope", model: "qwen3-omni-flash", base_url: null },
  general: {
    auto_launch: false,
    sound_feedback: true,
    floating_bar_position: "top_right",
    minimize_to_tray: true,
  },
  audio: { silence_threshold: 0.01, silence_duration_ms: 500 },
  shortcuts: {},
  language: { primary: "zh-CN", translate_target: "en" },
  dictionary: [],
};

describe("SettingsDialog", () => {
  beforeEach(() => {
    resetStores();
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_config") return DEFAULT_CONFIG;
      if (cmd === "get_api_key") return null;
      return undefined;
    });
  });

  it("关闭状态不渲染 Dialog 内容", () => {
    renderWithProviders(<SettingsDialog />);

    // Dialog 未打开 → 无 tab 角色元素
    expect(screen.queryAllByRole("tab")).toHaveLength(0);
  });

  it("打开状态渲染 tab 列表", async () => {
    useUIStore.getState().openSettings();
    renderWithProviders(<SettingsDialog />);

    // 等待 4 个 tab 渲染（账户、设置、个性化、关于）
    await waitFor(() => {
      expect(screen.getAllByRole("tab")).toHaveLength(4);
    });
  });

  it("打开后关闭 settingsOpen 状态", async () => {
    useUIStore.getState().openSettings();
    expect(useUIStore.getState().settingsOpen).toBe(true);

    useUIStore.getState().closeSettings();
    expect(useUIStore.getState().settingsOpen).toBe(false);
  });
});
