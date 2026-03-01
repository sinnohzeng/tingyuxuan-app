import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useUIStore } from "../../../shared/stores/uiStore";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn().mockResolvedValue([]),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import { useDictionary } from "./useDictionary";

describe("useDictionary", () => {
  beforeEach(() => {
    mockInvoke.mockReset().mockResolvedValue([]);
    useUIStore.setState({ toasts: [] });
  });

  it("初始加载词典", async () => {
    mockInvoke.mockResolvedValue(["hello", "world"]);

    const { result } = renderHook(() => useDictionary());

    await waitFor(() => {
      expect(result.current.words).toEqual(["hello", "world"]);
      expect(result.current.isLoading).toBe(false);
    });
  });

  it("添加词汇（乐观更新）", async () => {
    mockInvoke
      .mockResolvedValueOnce(["existing"]) // get_dictionary
      .mockResolvedValueOnce(undefined); // add_dictionary_word

    const { result } = renderHook(() => useDictionary());

    await waitFor(() => {
      expect(result.current.words).toEqual(["existing"]);
    });

    await act(() => result.current.addWord("new"));

    expect(result.current.words).toContain("new");
  });

  it("重复词汇不添加", async () => {
    mockInvoke.mockResolvedValue(["hello"]);

    const { result } = renderHook(() => useDictionary());

    await waitFor(() => {
      expect(result.current.words).toEqual(["hello"]);
    });

    await act(() => result.current.addWord("hello"));

    // 仍然只有一个
    expect(result.current.words).toEqual(["hello"]);
    // 没有调用 add_dictionary_word
    expect(mockInvoke).not.toHaveBeenCalledWith("add_dictionary_word", expect.anything());
  });

  it("添加失败回滚 + toast", async () => {
    mockInvoke
      .mockResolvedValueOnce(["existing"]) // get_dictionary
      .mockRejectedValueOnce(new Error("db error")); // add fails

    const { result } = renderHook(() => useDictionary());

    await waitFor(() => {
      expect(result.current.words).toEqual(["existing"]);
    });

    await act(() => result.current.addWord("new"));

    await waitFor(() => {
      // 回滚
      expect(result.current.words).not.toContain("new");
      // toast
      const toasts = useUIStore.getState().toasts;
      expect(toasts[0].title).toContain("添加词汇失败");
    });
  });

  it("删除词汇（乐观更新 + 失败回滚）", async () => {
    mockInvoke
      .mockResolvedValueOnce(["a", "b"]) // get_dictionary
      .mockRejectedValueOnce(new Error("fail")); // remove fails

    const { result } = renderHook(() => useDictionary());

    await waitFor(() => {
      expect(result.current.words).toEqual(["a", "b"]);
    });

    await act(() => result.current.removeWord("a"));

    await waitFor(() => {
      // 回滚
      expect(result.current.words).toContain("a");
    });
  });
});
