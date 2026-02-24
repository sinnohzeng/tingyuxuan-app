import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ResultPanel from "./ResultPanel";

// Mock Tauri window API
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setSize: () => Promise.resolve(),
  }),
  LogicalSize: class {
    constructor(public width: number, public height: number) {}
  },
}));

describe("ResultPanel", () => {
  const defaultProps = {
    result: "这是 AI 的回复内容",
    onCopy: vi.fn(),
    onInsert: vi.fn(),
    onDismiss: vi.fn(),
  };

  it("renders result text and action buttons", () => {
    render(<ResultPanel {...defaultProps} />);

    expect(screen.getByText("AI 助手")).toBeInTheDocument();
    expect(screen.getByText("这是 AI 的回复内容")).toBeInTheDocument();
    expect(screen.getByText("复制")).toBeInTheDocument();
    expect(screen.getByText("插入到光标")).toBeInTheDocument();
  });

  it("calls onCopy when copy button is clicked", () => {
    const onCopy = vi.fn();
    render(<ResultPanel {...defaultProps} onCopy={onCopy} />);

    fireEvent.click(screen.getByText("复制"));
    expect(onCopy).toHaveBeenCalledOnce();
  });

  it("calls onDismiss when close button is clicked", () => {
    const onDismiss = vi.fn();
    render(<ResultPanel {...defaultProps} onDismiss={onDismiss} />);

    fireEvent.click(screen.getByTitle("关闭"));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("calls onInsert when insert button is clicked", () => {
    const onInsert = vi.fn();
    render(<ResultPanel {...defaultProps} onInsert={onInsert} />);

    fireEvent.click(screen.getByText("插入到光标"));
    expect(onInsert).toHaveBeenCalledOnce();
  });
});
