import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { useAppStore } from "../shared/stores/appStore";
import FloatingBar from "../features/recording/FloatingBar";

const mockInvoke = vi.fn(() => Promise.resolve());

// Mock Tauri APIs — FloatingBar dynamically imports these
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
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
    vi.useRealTimers();
    mockInvoke.mockClear();
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

  it("shows starting indicator", () => {
    useAppStore.setState({ recordingState: "starting" });
    render(<FloatingBar />);

    expect(screen.getByText("正在启动...")).toBeInTheDocument();
  });

  it("shows thinking spinner", () => {
    useAppStore.setState({ recordingState: "thinking" });
    render(<FloatingBar />);

    expect(screen.getByText("思考中...")).toBeInTheDocument();
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

    expect(screen.getByText("已注入文本")).toBeInTheDocument();
  });

  it("shows translate mode label", () => {
    useAppStore.setState({
      recordingState: "recording",
      recordingMode: "translate",
    });
    render(<FloatingBar />);

    expect(screen.getByText("翻译模式")).toBeInTheDocument();
  });

  it("shows 60s countdown popup at 4 minutes", async () => {
    vi.useFakeTimers();
    useAppStore.setState({ recordingState: "recording", recordingMode: "dictate" });
    render(<FloatingBar />);

    await act(async () => {
      vi.advanceTimersByTime(240_000);
      await Promise.resolve();
    });

    expect(screen.getByText("录音时长限制")).toBeInTheDocument();
    expect(screen.getByText("倒计时 1:00")).toBeInTheDocument();
  });

  it("auto-stops recording at 5 minutes", async () => {
    vi.useFakeTimers();
    useAppStore.setState({ recordingState: "recording", recordingMode: "dictate" });
    render(<FloatingBar />);

    await act(async () => {
      vi.advanceTimersByTime(300_000);
      await Promise.resolve();
    });

    expect(useAppStore.getState().recordingState).toBe("thinking");
  });
});
