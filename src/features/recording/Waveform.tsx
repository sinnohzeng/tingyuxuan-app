import { useEffect, useRef } from "react";

interface WaveformProps {
  levels: number[];
  isActive: boolean;
}

const BAR_COUNT = 18;
const BAR_GAP = 4;
const MIN_HALF_HEIGHT = 1.5;
const LERP_FACTOR = 0.28;
const LEVEL_GAIN = 2.2;

export default function Waveform({ levels, isActive }: WaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const prevHeightsRef = useRef<number[]>(new Array(BAR_COUNT).fill(MIN_HALF_HEIGHT));
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

    const availableWidth = width - BAR_GAP * (BAR_COUNT - 1);
    const barWidth = Math.max(2, availableWidth / BAR_COUNT);
    const centerY = height / 2;
    const maxHalfHeight = Math.max(4, height * 0.44);

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Middle guide line (Typeless-like center axis)
    ctx.fillStyle = isActive ? "rgba(255,255,255,0.24)" : "rgba(148,163,184,0.24)";
    ctx.fillRect(0, centerY - 0.5, width, 1);

    // Draw mirrored bars around the center axis.
    const prevHeights = prevHeightsRef.current;
    for (let i = 0; i < BAR_COUNT; i++) {
      const levelIndex = Math.floor((i / BAR_COUNT) * Math.max(levels.length, 1));
      const targetLevel = levels[levelIndex] ?? 0;
      const amplified = Math.min(1, targetLevel * LEVEL_GAIN);
      const targetHalfHeight = Math.max(
        MIN_HALF_HEIGHT,
        amplified * maxHalfHeight,
      );

      const currentHalfHeight =
        prevHeights[i] + (targetHalfHeight - prevHeights[i]) * LERP_FACTOR;
      prevHeights[i] = currentHalfHeight;

      const x = i * (barWidth + BAR_GAP);
      const topY = centerY - currentHalfHeight - 1;
      const bottomY = centerY + 1;
      ctx.fillStyle = isActive ? "#f8fafc" : "#94a3b8";

      if (typeof ctx.roundRect === "function") {
        ctx.beginPath();
        ctx.roundRect(x, topY, barWidth, currentHalfHeight, barWidth / 2);
        ctx.roundRect(x, bottomY, barWidth, currentHalfHeight, barWidth / 2);
        ctx.fill();
      } else {
        ctx.fillRect(x, topY, barWidth, currentHalfHeight);
        ctx.fillRect(x, bottomY, barWidth, currentHalfHeight);
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
