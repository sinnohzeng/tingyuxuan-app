import { useEffect, useRef } from "react";

interface WaveformProps {
  levels: number[];
  isActive: boolean;
}

const BAR_COUNT = 30;
const BAR_GAP = 2;
const MIN_BAR_HEIGHT = 2;
const LERP_FACTOR = 0.3;

export default function Waveform({ levels, isActive }: WaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const prevHeightsRef = useRef<number[]>(new Array(BAR_COUNT).fill(0));

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const width = rect.width;
    const height = rect.height;
    const barWidth = (width - BAR_GAP * (BAR_COUNT - 1)) / BAR_COUNT;
    const maxBarHeight = height - 4; // 2px padding top/bottom

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Create gradient
    const gradient = ctx.createLinearGradient(0, 0, width, 0);
    if (isActive) {
      gradient.addColorStop(0, "#3b82f6"); // blue-500
      gradient.addColorStop(1, "#8b5cf6"); // violet-500
    } else {
      gradient.addColorStop(0, "#6b7280"); // gray-500
      gradient.addColorStop(1, "#9ca3af"); // gray-400
    }

    // Map levels to bar heights with interpolation
    const prevHeights = prevHeightsRef.current;
    for (let i = 0; i < BAR_COUNT; i++) {
      // Sample from levels array, or use 0 if not enough data
      const levelIndex = Math.floor((i / BAR_COUNT) * Math.max(levels.length, 1));
      const targetLevel = levels[levelIndex] ?? 0;
      const targetHeight = Math.max(
        MIN_BAR_HEIGHT,
        targetLevel * maxBarHeight
      );

      // Lerp from previous height for smooth animation
      const currentHeight =
        prevHeights[i] + (targetHeight - prevHeights[i]) * LERP_FACTOR;
      prevHeights[i] = currentHeight;

      const x = i * (barWidth + BAR_GAP);
      const y = (height - currentHeight) / 2;

      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.roundRect(x, y, barWidth, currentHeight, barWidth / 2);
      ctx.fill();
    }
  }, [levels, isActive]);

  return (
    <canvas
      ref={canvasRef}
      className="w-full h-full"
      style={{ width: "100%", height: "100%" }}
    />
  );
}
