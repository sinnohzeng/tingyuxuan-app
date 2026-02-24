import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ErrorBoundary from "./ErrorBoundary";

// A component that throws on render
function BrokenComponent({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error("Test render error");
  }
  return <div>正常内容</div>;
}

describe("ErrorBoundary", () => {
  beforeEach(() => {
    // Suppress React error boundary console.error noise in test output
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("renders children when there is no error", () => {
    render(
      <ErrorBoundary>
        <div>正常内容</div>
      </ErrorBoundary>
    );
    expect(screen.getByText("正常内容")).toBeInTheDocument();
  });

  it("renders fallback UI when a child throws", () => {
    render(
      <ErrorBoundary>
        <BrokenComponent shouldThrow={true} />
      </ErrorBoundary>
    );
    expect(screen.getByText("出了点问题")).toBeInTheDocument();
    expect(screen.getByText("Test render error")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
  });

  it("resets error state when retry is clicked", () => {
    // Use an external flag so the component always throws during the initial
    // render (including React 19's concurrent retry), then stops throwing
    // before we click the retry button.
    let shouldThrow = true;
    function Controllable() {
      if (shouldThrow) {
        throw new Error("Controlled error");
      }
      return <div>恢复成功</div>;
    }

    render(
      <ErrorBoundary>
        <Controllable />
      </ErrorBoundary>
    );

    // Should show error state
    expect(screen.getByText("出了点问题")).toBeInTheDocument();

    // Flip the flag so re-render after retry succeeds
    shouldThrow = false;
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(screen.getByText("恢复成功")).toBeInTheDocument();
  });
});
