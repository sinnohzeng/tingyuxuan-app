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
  const sizeRef = useRef({ width: 0, height: 0 });

  // 监听尺寸变化，避免每次 levels 更新都调用 getBoundingClientRect()
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    // 初始化尺寸
    const rect = canvas.getBoundingClientRect();
    sizeRef.current = { width: rect.width, height: rect.height };

    // ResizeObserver 可能在 jsdom 等测试环境中不可用
    if (typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        sizeRef.current = { width, height };
      }
    });
    observer.observe(canvas);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const { width, height } = sizeRef.current;
    if (width === 0 || height === 0) return;

    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx.scale(dpr, dpr);

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
      if (typeof ctx.roundRect === "function") {
        ctx.beginPath();
        ctx.roundRect(x, y, barWidth, currentHeight, barWidth / 2);
        ctx.fill();
      } else {
        ctx.fillRect(x, y, barWidth, currentHeight);
      }
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
