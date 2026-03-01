import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

const mockUnlisten = vi.fn();
const mockListen = vi.fn().mockResolvedValue(mockUnlisten);

vi.mock("@tauri-apps/api/event", () => ({
  listen: mockListen,
}));

import { useTauriEvent } from "./useTauriEvent";

describe("useTauriEvent", () => {
  beforeEach(() => {
    mockListen.mockClear().mockResolvedValue(mockUnlisten);
    mockUnlisten.mockClear();
  });

  it("注册事件监听器", async () => {
    const handler = vi.fn();
    renderHook(() => useTauriEvent("test-event", handler));

    await vi.waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith("test-event", handler);
    });
  });

  it("卸载时调用 unlisten", async () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent("test-event", handler));

    await vi.waitFor(() => {
      expect(mockListen).toHaveBeenCalled();
    });

    unmount();

    expect(mockUnlisten).toHaveBeenCalled();
  });

  it("事件名变化时重新注册", async () => {
    const handler = vi.fn();
    const { rerender } = renderHook(
      ({ event }) => useTauriEvent(event, handler),
      { initialProps: { event: "event-a" } },
    );

    await vi.waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith("event-a", handler);
    });

    rerender({ event: "event-b" });

    await vi.waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith("event-b", handler);
    });
    // 旧监听器被清理
    expect(mockUnlisten).toHaveBeenCalled();
  });
});
