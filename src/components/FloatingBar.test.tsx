import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useAppStore } from "../stores/appStore";
import FloatingBar from "./FloatingBar";

// Mock Tauri APIs — FloatingBar dynamically imports these
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    hide: () => Promise.resolve(),
    show: () => Promise.resolve(),
    setSize: () => Promise.resolve(),
  }),
  LogicalSize: class {
    constructor(public width: number, public height: number) {}
  },
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: {
    getByLabel: vi.fn(() => Promise.resolve(null)),
  },
}));

describe("FloatingBar", () => {
  beforeEach(() => {
    useAppStore.getState().reset();
  });

  it("renders nothing when idle", () => {
    const { container } = render(<FloatingBar />);
    expect(container.innerHTML).toBe("");
  });

  it("shows recording UI with mode label and cancel/confirm buttons", () => {
    useAppStore.setState({ recordingState: "recording", recordingMode: "dictate" });
    render(<FloatingBar />);

    expect(screen.getByText("听写")).toBeInTheDocument();
    expect(screen.getByTitle("取消 (Esc)")).toBeInTheDocument();
    expect(screen.getByTitle("完成")).toBeInTheDocument();
  });

  it("shows processing spinner", () => {
    useAppStore.setState({ recordingState: "processing" });
    render(<FloatingBar />);

    expect(screen.getByText("处理中...")).toBeInTheDocument();
  });

  it("shows error state with error panel", () => {
    useAppStore.setState({
      recordingState: "error",
      errorMessage: "API 连接失败",
      errorAction: "Retry",
    });
    render(<FloatingBar />);

    expect(screen.getByText("API 连接失败")).toBeInTheDocument();
  });

  it("shows done state with checkmark for non-AI modes", () => {
    useAppStore.setState({
      recordingState: "done",
      recordingMode: "dictate",
    });
    render(<FloatingBar />);

    expect(screen.getByText("完成")).toBeInTheDocument();
  });

  it("shows translate mode label", () => {
    useAppStore.setState({
      recordingState: "recording",
      recordingMode: "translate",
    });
    render(<FloatingBar />);

    expect(screen.getByText("翻译模式")).toBeInTheDocument();
  });
});
