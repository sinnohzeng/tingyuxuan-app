import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useUIStore } from "../../../shared/stores/uiStore";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn().mockResolvedValue([]),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import { useHistory } from "./useHistory";

describe("useHistory", () => {
  beforeEach(() => {
    mockInvoke.mockReset().mockResolvedValue([]);
    useUIStore.setState({ toasts: [] });
  });

  it("初始加载调用 get_history_page", async () => {
    renderHook(() => useHistory());

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_history_page", { limit: 20, offset: 0 });
    });
  });

  it("hasMore 在返回满页时为 true", async () => {
    const page = Array.from({ length: 20 }, (_, i) => ({
      id: String(i),
      session_id: `s${i}`,
      raw_text: `text ${i}`,
      processed_text: `text ${i}`,
      mode: "dictate",
      status: "completed",
      duration_ms: 1000,
      char_count: 5,
      timestamp: "2026-01-01T00:00:00Z",
    }));
    mockInvoke.mockResolvedValue(page);

    const { result } = renderHook(() => useHistory());

    await waitFor(() => {
      expect(result.current.hasMore).toBe(true);
    });
  });

  it("deleteRecord 从列表中移除记录", async () => {
    const records = [
      { id: "1", session_id: "s1", raw_text: "a", processed_text: "a", mode: "dictate", status: "completed", duration_ms: 1000, char_count: 1, timestamp: "2026-01-01T00:00:00Z" },
      { id: "2", session_id: "s2", raw_text: "b", processed_text: "b", mode: "dictate", status: "completed", duration_ms: 1000, char_count: 1, timestamp: "2026-01-01T00:00:00Z" },
    ];
    mockInvoke
      .mockResolvedValueOnce(records) // initial load
      .mockResolvedValueOnce(undefined); // delete

    const { result } = renderHook(() => useHistory());

    await waitFor(() => {
      expect(result.current.records).toHaveLength(2);
    });

    await act(() => result.current.deleteRecord("1"));

    expect(result.current.records).toHaveLength(1);
    expect(result.current.records[0].id).toBe("2");
  });

  it("加载失败触发 toast", async () => {
    mockInvoke.mockRejectedValue(new Error("db error"));

    renderHook(() => useHistory());

    await waitFor(() => {
      const toasts = useUIStore.getState().toasts;
      expect(toasts).toHaveLength(1);
      expect(toasts[0].title).toBe("加载历史记录失败");
    });
  });
});
