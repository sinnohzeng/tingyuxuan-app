import { useEffect, useRef, useCallback, useState } from "react";
import { useAppStore } from "../../shared/stores/appStore";
import type { PipelineEvent, UserAction } from "../../shared/lib/types";
import { createLogger, setLogSession } from "../../shared/lib/logger";
import { trackEvent } from "../../shared/lib/telemetry";
import { useSoundEffect } from "./hooks/useSoundEffect";
import Waveform from "./Waveform";
import ErrorPanel from "./ErrorPanel";
import ResultPanel from "./ResultPanel";

const log = createLogger("FloatingBar");

/** Mode display labels */
const MODE_LABELS: Record<string, string> = {
  dictate: "听写",
  translate: "翻译模式",
  ai_assistant: "AI 助手",
  edit: "语音编辑",
};

const MAX_RECORDING_SECONDS = 300;
const COUNTDOWN_START_SECONDS = 240;

/** Format duration as M:SS */
function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** Format remaining seconds as M:SS for countdown. */
function formatCountdown(seconds: number): string {
  const safe = Math.max(0, seconds);
  const m = Math.floor(safe / 60);
  const s = Math.floor(safe % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** 根据状态计算浮动条容器的 className */
function barContainerClass(
  state: string,
  mode: string,
  aiResult: string | null,
): string {
  if (state === "error") return "w-[320px] min-h-[56px]";
  if (state === "done" && mode === "ai_assistant" && aiResult) {
    return "w-[420px] h-[360px] flex-col";
  }
  if (state === "recording") return "w-[252px] h-[60px]";
  return "w-[220px] h-[56px]";
}

/** 根据状态计算浮动条的样式 */
function barStyleClass(state: string): string {
  switch (state) {
    case "starting":
      return "bg-black/92 border border-white/15";
    case "recording":
      return "bg-[#0d0d10]/95 border border-white/35 shadow-[0_0_0_1px_rgba(255,255,255,0.12),0_10px_22px_rgba(0,0,0,0.42)]";
    case "thinking":
      return "bg-black/90 border border-white/15";
    case "done":
      return "bg-black/90 border border-white/15";
    case "cancelled":
      return "bg-black/90 border border-white/15";
    case "error":
      return "bg-black/90 border border-red-500/40 animate-error-shake";
    default:
      return "bg-black/90 border border-white/10";
  }
}

export default function FloatingBar() {
  const recordingState = useAppStore((s) => s.recordingState);
  const recordingMode = useAppStore((s) => s.recordingMode);
  const volumeLevels = useAppStore((s) => s.volumeLevels);
  const recordingDuration = useAppStore((s) => s.recordingDuration);
  const errorMessage = useAppStore((s) => s.errorMessage);
  const errorAction = useAppStore((s) => s.errorAction);
  const aiResult = useAppStore((s) => s.aiResult);
  const setRecordingState = useAppStore((s) => s.setRecordingState);
  const reset = useAppStore((s) => s.reset);

  const { playStartSound, playStopSound, playErrorSound } = useSoundEffect();
  const [showLimitPopup, setShowLimitPopup] = useState(false);

  const durationTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const prevStateRef = useRef(recordingState);
  const limitPopupShownRef = useRef(false);
  const autoStopTriggeredRef = useRef(false);

  const handleAutoStopAtLimit = useCallback(async () => {
    if (useAppStore.getState().recordingState !== "recording") {
      return;
    }
    log.info(`recording limit reached (${MAX_RECORDING_SECONDS}s), auto-stopping`);
    trackEvent("recording_limit_reached", {
      limit_seconds: MAX_RECORDING_SECONDS,
    });
    setRecordingState("thinking");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("stop_recording");
    } catch (error) {
      log.warn("auto stop invoke failed", { error });
      useAppStore.getState().setError("录音已达到 5 分钟上限，请重试", "Retry");
    }
  }, [setRecordingState]);

  // 音效：状态变化时播放
  useEffect(() => {
    const prev = prevStateRef.current;
    prevStateRef.current = recordingState;

    if (prev === recordingState) return;

    if (recordingState === "recording") {
      playStartSound();
    } else if (recordingState === "done") {
      playStopSound();
    } else if (recordingState === "error" || recordingState === "cancelled") {
      playErrorSound();
    }
  }, [recordingState, playStartSound, playStopSound, playErrorSound]);

  // Start duration timer when recording
  useEffect(() => {
    if (recordingState === "recording") {
      const store = useAppStore.getState();
      store.setRecordingDuration(0);
      setShowLimitPopup(false);
      limitPopupShownRef.current = false;
      autoStopTriggeredRef.current = false;
      durationTimerRef.current = setInterval(() => {
        const nextDuration = useAppStore.getState().recordingDuration + 1;
        const clampedDuration = Math.min(nextDuration, MAX_RECORDING_SECONDS);
        useAppStore.getState().setRecordingDuration(clampedDuration);

        if (
          clampedDuration >= COUNTDOWN_START_SECONDS &&
          !limitPopupShownRef.current
        ) {
          limitPopupShownRef.current = true;
          setShowLimitPopup(true);
        }

        if (
          clampedDuration >= MAX_RECORDING_SECONDS &&
          !autoStopTriggeredRef.current
        ) {
          autoStopTriggeredRef.current = true;
          void handleAutoStopAtLimit();
        }
      }, 1000);
    } else {
      if (durationTimerRef.current) {
        clearInterval(durationTimerRef.current);
        durationTimerRef.current = null;
      }
      setShowLimitPopup(false);
    }
    return () => {
      if (durationTimerRef.current) clearInterval(durationTimerRef.current);
    };
  }, [recordingState, handleAutoStopAtLimit]);

  // Auto-hide after done/cancelled (except AI assistant — it shows result panel).
  useEffect(() => {
    const isAiDone = recordingState === "done" && recordingMode === "ai_assistant";
    if ((recordingState === "done" && !isAiDone) || recordingState === "cancelled") {
      hideTimerRef.current = setTimeout(() => {
        reset();
      }, 1500);
    }
    return () => {
      if (hideTimerRef.current) clearTimeout(hideTimerRef.current);
    };
  }, [recordingState, recordingMode, reset]);

  // 浮动条窗口可见性：前端作为唯一职责方
  useEffect(() => {
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        if (recordingState === "idle") {
          win.hide().catch(() => {});
        } else {
          win.show().catch(() => {});
        }
      })
      .catch(() => {});
  }, [recordingState]);

  // Listen for Tauri pipeline events and shortcut actions
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let cleaned = false;

    async function setupListeners() {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const store = useAppStore.getState();

        // Pipeline events from the Rust backend
        const u1 = await listen<PipelineEvent>("pipeline-event", (event) => {
          const data = event.payload;
          if (data.type !== "VolumeUpdate") {
            log.debug(`pipeline-event: ${data.type}`, data);
          }
          switch (data.type) {
            case "RecorderStarting":
              store.setRecordingMode(
                data.mode === "dictate" || data.mode === "translate" || data.mode === "ai_assistant" || data.mode === "edit"
                  ? data.mode
                  : "dictate",
              );
              store.setRecordingState("starting");
              break;
            case "RecordingStarted":
              setLogSession(data.session_id);
              log.info(`session started: mode=${data.mode}`);
              store.setSessionId(data.session_id);
              store.setRecordingMode(
                data.mode === "dictate" || data.mode === "translate" || data.mode === "ai_assistant" || data.mode === "edit"
                  ? data.mode
                  : "dictate",
              );
              store.setRecordingState("recording");
              break;
            case "VolumeUpdate":
              store.setVolumeLevels(data.levels);
              break;
            case "RecordingStopped":
              // Don't change state yet; thinking starts next
              break;
            case "ThinkingStarted":
            case "ProcessingStarted":
              store.setRecordingState("thinking");
              break;
            case "ProcessingComplete":
              log.info("thinking complete");
              if (useAppStore.getState().recordingMode === "ai_assistant") {
                store.setAiResult(data.processed_text);
              }
              store.setRecordingState("done");
              break;
            case "Error": {
              log.warn(`pipeline error: ${data.message}`, { action: data.user_action });
              const knownActions: UserAction[] = ["Retry", "CheckApiKey", "WaitAndRetry", "CheckMicrophone"];
              const action = knownActions.includes(data.user_action as UserAction)
                ? (data.user_action as UserAction)
                : "Retry";
              store.setError(data.message, action);
              break;
            }
            case "NetworkStatusChanged":
              store.setIsOnline(data.online);
              break;
            case "RecordingCancelled":
              store.setRecordingState("cancelled");
              break;
          }
        });
        // 如果 cleanup 已在 await 期间执行，立即取消注册。
        if (cleaned) { u1(); return; }
        unlisteners.push(u1);

        // 注意：shortcut-action 监听已移至 MainLayout（主窗口始终加载，避免隐藏窗口事件丢失）
      } catch {
        // Not running in Tauri - that's fine for dev mode
      }
    }

    setupListeners();
    return () => {
      cleaned = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  const handleCancel = useCallback(async () => {
    log.info("user action: cancel");
    trackEvent("user_action", { action: "cancel" });
    setShowLimitPopup(false);
    setRecordingState("cancelled");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("cancel_recording");
    } catch {
      /* dev mode */
    }
  }, [setRecordingState]);

  const handleConfirm = useCallback(async () => {
    log.info("user action: confirm");
    trackEvent("user_action", { action: "confirm" });
    setShowLimitPopup(false);
    setRecordingState("thinking");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("stop_recording");
    } catch {
      /* dev mode */
    }
  }, [setRecordingState]);

  const handleDismiss = useCallback(() => {
    trackEvent("user_action", { action: "dismiss_error" });
    reset();
  }, [reset]);

  const handleOpenSettings = useCallback(async () => {
    trackEvent("user_action", { action: "open_settings" });
    try {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const win = await WebviewWindow.getByLabel("settings");
      if (win) {
        await win.show();
        await win.setFocus();
      }
    } catch {
      /* dev mode */
    }
    reset();
  }, [reset]);

  const handleOpenMicSettings = useCallback(async () => {
    trackEvent("user_action", { action: "open_mic_settings" });
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_permission_settings", { target: "microphone" });
    } catch {
      /* dev mode */
    }
  }, []);

  // Don't render anything when idle
  if (recordingState === "idle") {
    return null;
  }

  const countdownSeconds = MAX_RECORDING_SECONDS - recordingDuration;
  const showCountdown = recordingState === "recording" && recordingDuration >= COUNTDOWN_START_SECONDS;

  return (
    <div className="floating-bar-window h-screen w-screen flex items-center justify-center">
      <div
        className={`
          relative flex items-center
          ${recordingState === "recording" ? "rounded-full" : "rounded-2xl"}
          backdrop-blur-md animate-bar-enter
          ${barStyleClass(recordingState)}
          ${barContainerClass(recordingState, recordingMode, aiResult)}
        `}
      >
        {/* Recording state */}
        {recordingState === "recording" && (
          <>
            {showLimitPopup && (
              <div
                className="absolute -top-20 left-1/2 -translate-x-1/2 w-[300px] rounded-2xl
                           border border-amber-300/35 bg-black/88 shadow-[0_10px_30px_rgba(0,0,0,0.45)] px-3 py-2"
                role="alert"
                aria-live="polite"
              >
                <div className="flex items-start gap-2">
                  <div className="mt-0.5 w-5 h-5 rounded-full border border-amber-300/70 text-amber-200 text-[11px] flex items-center justify-center">
                    !
                  </div>
                  <div className="flex-1">
                    <div className="text-[12px] text-amber-100 font-medium">录音时长限制</div>
                    <div className="text-[11px] text-white/70">
                      MVP 仅支持单次录音小于等于 5 分钟，剩余 {formatCountdown(countdownSeconds)}
                    </div>
                  </div>
                  <button
                    onClick={() => setShowLimitPopup(false)}
                    className="text-white/50 hover:text-white/80 text-xs leading-none"
                    aria-label="关闭时长提示"
                    title="关闭提示"
                  >
                    ✕
                  </button>
                </div>
              </div>
            )}

            {/* Cancel button */}
            <button
              onClick={handleCancel}
              className="flex-none w-10 h-10 flex items-center justify-center rounded-full ml-2.5
                         border border-white/25 bg-white/12 text-white/80
                         hover:bg-white/20 hover:text-white transition-colors"
              title="取消 (Esc)"
              aria-label="取消录制"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>

            {/* Waveform + info */}
            <div className="flex-1 flex flex-col items-center px-2.5">
              <div className="w-full h-9">
                <Waveform levels={volumeLevels} isActive={true} />
              </div>
              <div className="flex items-center gap-2 text-[10px] text-white/70 mt-0.5">
                <span>{MODE_LABELS[recordingMode] || recordingMode}</span>
                <span className="text-white/25">•</span>
                <span className="tabular-nums">{formatDuration(recordingDuration)}</span>
                {showCountdown && (
                  <>
                    <span className="text-white/25">•</span>
                    <span className="tabular-nums text-amber-200">
                      倒计时 {formatCountdown(countdownSeconds)}
                    </span>
                  </>
                )}
              </div>
            </div>

            {/* Confirm button */}
            <button
              onClick={handleConfirm}
              className="flex-none w-10 h-10 flex items-center justify-center rounded-full mr-2.5
                         border border-white/25 bg-white text-black
                         hover:bg-white/90 transition-colors"
              title="完成"
              aria-label="停止录制"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            </button>
          </>
        )}

        {/* Starting state */}
        {recordingState === "starting" && (
          <div className="flex-1 flex items-center justify-center gap-3 px-4">
            <div className="w-4 h-4 border-2 border-white/60 border-t-transparent rounded-full animate-spin" />
            <span className="text-white/85 text-sm">正在启动...</span>
          </div>
        )}

        {/* Thinking state */}
        {recordingState === "thinking" && (
          <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4">
            <div className="flex items-center gap-3">
              <div className="w-5 h-5 border-2 border-white/60 border-t-transparent rounded-full animate-spin" />
              <span className="text-white/90 text-sm">思考中...</span>
            </div>
            <div className="w-full h-1 bg-white/10 rounded-full overflow-hidden">
              <div className="h-full w-1/3 bg-white/40 rounded-full animate-progress-pulse" />
            </div>
          </div>
        )}

        {/* Done state — AI assistant shows result panel */}
        {recordingState === "done" && recordingMode === "ai_assistant" && aiResult && (
          <ResultPanel
            result={aiResult}
            onCopy={() => navigator.clipboard.writeText(aiResult)}
            onInsert={async () => {
              trackEvent("user_action", { action: "result_insert" });
              try {
                const { invoke } = await import("@tauri-apps/api/core");
                await invoke("inject_text", { text: aiResult });
              } catch { /* dev mode */ }
              reset();
            }}
            onDismiss={reset}
          />
        )}
        {/* Done state — normal modes */}
        {recordingState === "done" && !(recordingMode === "ai_assistant" && aiResult) && (
          <div className="flex-1 flex items-center justify-center gap-2 px-4">
            <svg className="w-5 h-5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
            <span className="text-white text-sm">已注入文本</span>
          </div>
        )}

        {/* Cancelled state */}
        {recordingState === "cancelled" && (
          <div className="flex-1 flex items-center justify-center gap-2 px-4">
            <span className="w-5 h-5 rounded-full border border-blue-400 text-blue-300 text-xs font-semibold flex items-center justify-center">
              i
            </span>
            <span className="text-white text-sm font-medium">转录已取消</span>
          </div>
        )}

        {/* Error state */}
        {recordingState === "error" && errorMessage && errorAction && (
          <ErrorPanel
            message={errorMessage}
            action={errorAction}
            onDismiss={handleDismiss}
            onOpenSettings={handleOpenSettings}
            onOpenMicSettings={handleOpenMicSettings}
          />
        )}
      </div>
    </div>
  );
}
