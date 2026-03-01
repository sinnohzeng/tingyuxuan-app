import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen } from "@testing-library/react";
import { renderWithProviders, resetStores } from "../../test-utils";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn().mockResolvedValue([]),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import HistoryPage from "./HistoryPage";

describe("HistoryPage", () => {
  beforeEach(() => {
    resetStores();
    mockInvoke.mockReset().mockResolvedValue([]);
  });

  it("显示标题", async () => {
    renderWithProviders(<HistoryPage />, { initialEntries: ["/main/history"] });

    expect(await screen.findByText("历史记录")).toBeInTheDocument();
  });

  it("显示搜索框", async () => {
    renderWithProviders(<HistoryPage />, { initialEntries: ["/main/history"] });

    expect(await screen.findByPlaceholderText("搜索历史记录…")).toBeInTheDocument();
  });

  it("有记录时显示清空按钮", async () => {
    mockInvoke.mockResolvedValue([
      {
        id: "1",
        session_id: "s1",
        raw_text: "你好",
        processed_text: "你好",
        mode: "dictate",
        status: "completed",
        duration_ms: 1000,
        char_count: 2,
        timestamp: "2026-01-01T00:00:00Z",
      },
    ]);

    renderWithProviders(<HistoryPage />, { initialEntries: ["/main/history"] });

    expect(await screen.findByText("清空")).toBeInTheDocument();
  });
});
