import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen } from "@testing-library/react";
import { renderWithProviders, resetStores } from "../../test-utils";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import HomePage from "./HomePage";

describe("HomePage", () => {
  beforeEach(() => {
    resetStores();
    mockInvoke.mockReset();
  });

  it("显示加载态", () => {
    // fetchStats 永不 resolve → 保持加载态
    mockInvoke.mockReturnValue(new Promise(() => {}));
    renderWithProviders(<HomePage />, { initialEntries: ["/main"] });

    expect(screen.getByText("加载统计中…")).toBeInTheDocument();
  });

  it("显示空状态", async () => {
    mockInvoke.mockResolvedValue({
      total_sessions: 0,
      successful_sessions: 0,
      total_duration_ms: 0,
      total_char_count: 0,
      average_speed_cpm: 0,
      estimated_time_saved_ms: 0,
      dictionary_utilization: 0,
    });

    renderWithProviders(<HomePage />, { initialEntries: ["/main"] });

    expect(await screen.findByText("欢迎使用听语轩")).toBeInTheDocument();
  });

  it("显示统计卡片", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_dashboard_stats") {
        return {
          total_sessions: 10,
          successful_sessions: 8,
          total_duration_ms: 600_000,
          total_char_count: 1234,
          average_speed_cpm: 120,
          estimated_time_saved_ms: 300_000,
          dictionary_utilization: 0.5,
        };
      }
      if (cmd === "get_history_page") return [];
      return undefined;
    });

    renderWithProviders(<HomePage />, { initialEntries: ["/main"] });

    expect(await screen.findByText("概览")).toBeInTheDocument();
    expect(screen.getByText("1,234")).toBeInTheDocument(); // total_char_count
  });
});
