import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ErrorPanel from "../features/recording/ErrorPanel";

describe("ErrorPanel", () => {
  const defaultProps = {
    message: "API 连接失败",
    action: "Retry" as const,
    onDismiss: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenMicSettings: vi.fn(),
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
    const onDismiss = vi.fn();
    render(<ErrorPanel {...defaultProps} action="Retry" onDismiss={onDismiss} />);

    const retryButton = screen.getByText("重试");
    expect(retryButton).toBeInTheDocument();
    fireEvent.click(retryButton);
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("renders dismiss button for Retry action and calls handler", () => {
    const onDismiss = vi.fn();
    render(<ErrorPanel {...defaultProps} action="Retry" onDismiss={onDismiss} />);

    const dismissButton = screen.getByText("稍后处理");
    expect(dismissButton).toBeInTheDocument();
    fireEvent.click(dismissButton);
    expect(onDismiss).toHaveBeenCalled();
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

  it("renders CheckMicrophone action with mic settings button", () => {
    const onOpenMicSettings = vi.fn();
    render(<ErrorPanel {...defaultProps} action="CheckMicrophone" onOpenMicSettings={onOpenMicSettings} />);

    const button = screen.getByText("打开麦克风设置");
    expect(button).toBeInTheDocument();
    fireEvent.click(button);
    expect(onOpenMicSettings).toHaveBeenCalledOnce();
  });
});
