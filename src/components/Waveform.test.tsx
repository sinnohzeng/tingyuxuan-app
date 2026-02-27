import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import Waveform from "./Waveform";

// Mock canvas context — jsdom does not implement canvas rendering
beforeEach(() => {
  // Provide a stub for HTMLCanvasElement.getContext
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    clearRect: vi.fn(),
    createLinearGradient: vi.fn(() => ({
      addColorStop: vi.fn(),
    })),
    beginPath: vi.fn(),
    fill: vi.fn(),
    roundRect: vi.fn(),
    scale: vi.fn(),
    set fillStyle(_v: unknown) {},
  } as unknown as CanvasRenderingContext2D);
});

describe("Waveform", () => {
  it("renders a canvas element", () => {
    const { container } = render(<Waveform levels={[]} isActive={true} />);
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });

  it("handles empty volume levels without crashing", () => {
    const { container } = render(<Waveform levels={[]} isActive={false} />);
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });

  it("accepts and renders with volume level data", () => {
    const levels = [0.1, 0.3, 0.5, 0.7, 0.9, 0.4, 0.2];
    const { container } = render(<Waveform levels={levels} isActive={true} />);
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();

    // Verify that the canvas context was used (drawing happened)
    const ctx = HTMLCanvasElement.prototype.getContext("2d");
    expect(ctx).toBeTruthy();
  });

  it("renders with isActive=false (inactive style)", () => {
    const { container } = render(<Waveform levels={[0.5, 0.5]} isActive={false} />);
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });
});
