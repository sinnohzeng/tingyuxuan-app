import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen } from "@testing-library/react";
import { renderWithProviders, resetStores } from "../../test-utils";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn().mockResolvedValue([]),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import DictionaryPage from "./DictionaryPage";

describe("DictionaryPage", () => {
  beforeEach(() => {
    resetStores();
    mockInvoke.mockReset().mockResolvedValue([]);
  });

  it("显示标题和说明文字", async () => {
    renderWithProviders(<DictionaryPage />, { initialEntries: ["/main/dictionary"] });

    expect(await screen.findByText("个人词典")).toBeInTheDocument();
    expect(screen.getByText(/添加专业术语/)).toBeInTheDocument();
  });

  it("显示输入框和添加按钮", async () => {
    renderWithProviders(<DictionaryPage />, { initialEntries: ["/main/dictionary"] });

    expect(await screen.findByPlaceholderText("输入新词汇…")).toBeInTheDocument();
    expect(screen.getByText("添加")).toBeInTheDocument();
  });

  it("词汇列表渲染 Tag", async () => {
    mockInvoke.mockResolvedValue(["听语轩", "AI", "Tauri"]);

    renderWithProviders(<DictionaryPage />, { initialEntries: ["/main/dictionary"] });

    expect(await screen.findByText("听语轩")).toBeInTheDocument();
    expect(screen.getByText("AI")).toBeInTheDocument();
    expect(screen.getByText("Tauri")).toBeInTheDocument();
  });
});
