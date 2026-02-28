import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ErrorPanel from "./ErrorPanel";

describe("ErrorPanel", () => {
  const defaultProps = {
    message: "API 连接失败",
    action: "Retry" as const,
    rawTranscript: null,
    onRetry: vi.fn(),
    onInsertRaw: vi.fn(),
    onDismiss: vi.fn(),
    onOpenSettings: vi.fn(),
  };

  it("renders without crashing", () => {
    const { container } = render(<ErrorPanel {...defaultProps} />);
    expect(container.firstChild).toBeTruthy();
  });

  it("has role='alert' on the main container", () => {
    render(<ErrorPanel {...defaultProps} />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("displays the error message", () => {
    render(<ErrorPanel {...defaultProps} message="网络连接超时" />);
    expect(screen.getByText("网络连接超时")).toBeInTheDocument();
  });

  it("renders retry button for Retry action and calls handler", () => {
    const onRetry = vi.fn();
    render(<ErrorPanel {...defaultProps} action="Retry" onRetry={onRetry} />);

    const retryButton = screen.getByText("重试");
    expect(retryButton).toBeInTheDocument();
    fireEvent.click(retryButton);
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it("renders dismiss button for Retry action and calls handler", () => {
    const onDismiss = vi.fn();
    render(<ErrorPanel {...defaultProps} action="Retry" onDismiss={onDismiss} />);

    const dismissButton = screen.getByText("稍后处理");
    expect(dismissButton).toBeInTheDocument();
    fireEvent.click(dismissButton);
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("renders InsertRawOrRetry actions with raw transcript", () => {
    const onInsertRaw = vi.fn();
    render(
      <ErrorPanel
        {...defaultProps}
        action="InsertRawOrRetry"
        rawTranscript="原始文本"
        onInsertRaw={onInsertRaw}
      />,
    );

    const insertButton = screen.getByText("插入原始转写");
    expect(insertButton).toBeInTheDocument();
    fireEvent.click(insertButton);
    expect(onInsertRaw).toHaveBeenCalledOnce();

    expect(screen.getByText("重试润色")).toBeInTheDocument();
    expect(screen.getByText("关闭")).toBeInTheDocument();
  });

  it("renders CheckApiKey action with settings button", () => {
    const onOpenSettings = vi.fn();
    render(
      <ErrorPanel {...defaultProps} action="CheckApiKey" onOpenSettings={onOpenSettings} />,
    );

    const settingsButton = screen.getByText("前往设置");
    expect(settingsButton).toBeInTheDocument();
    fireEvent.click(settingsButton);
    expect(onOpenSettings).toHaveBeenCalledOnce();
  });

  it("renders CheckMicrophone action with acknowledge button", () => {
    const onDismiss = vi.fn();
    render(<ErrorPanel {...defaultProps} action="CheckMicrophone" onDismiss={onDismiss} />);

    const button = screen.getByText("知道了");
    expect(button).toBeInTheDocument();
    fireEvent.click(button);
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
