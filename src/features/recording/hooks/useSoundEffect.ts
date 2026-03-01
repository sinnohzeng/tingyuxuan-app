/**
 * 录音状态音效 hook — 使用 Web Audio API 合成短促提示音，零外部依赖。
 */
import { useCallback, useRef } from "react";

/** 获取或复用 AudioContext（浏览器限制：需在用户交互后才能 resume） */
function getAudioCtx(ref: React.RefObject<AudioContext | null>): AudioContext {
  if (!ref.current) {
    ref.current = new AudioContext();
  }
  if (ref.current.state === "suspended") {
    ref.current.resume().catch(() => {});
  }
  return ref.current;
}

/** 播放一个短促合成音 */
function playTone(
  ctx: AudioContext,
  startFreq: number,
  endFreq: number,
  durationMs: number,
  volume = 0.15,
) {
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = "sine";
  osc.frequency.setValueAtTime(startFreq, ctx.currentTime);
  osc.frequency.linearRampToValueAtTime(endFreq, ctx.currentTime + durationMs / 1000);

  gain.gain.setValueAtTime(volume, ctx.currentTime);
  gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + durationMs / 1000);

  osc.connect(gain);
  gain.connect(ctx.destination);
  osc.start();
  osc.stop(ctx.currentTime + durationMs / 1000);
}

export function useSoundEffect() {
  const ctxRef = useRef<AudioContext | null>(null);

  /** 录音开始：上升短促 beep（200ms, 880Hz→1047Hz） */
  const playStartSound = useCallback(() => {
    try {
      const ctx = getAudioCtx(ctxRef);
      playTone(ctx, 880, 1047, 200, 0.12);
    } catch { /* 静默降级 */ }
  }, []);

  /** 录音完成：下降 ding（200ms, 1047Hz→880Hz） */
  const playStopSound = useCallback(() => {
    try {
      const ctx = getAudioCtx(ctxRef);
      playTone(ctx, 1047, 880, 200, 0.12);
    } catch { /* 静默降级 */ }
  }, []);

  /** 错误/取消：低沉短音（150ms, 440Hz） */
  const playErrorSound = useCallback(() => {
    try {
      const ctx = getAudioCtx(ctxRef);
      playTone(ctx, 440, 380, 150, 0.10);
    } catch { /* 静默降级 */ }
  }, []);

  return { playStartSound, playStopSound, playErrorSound };
}
