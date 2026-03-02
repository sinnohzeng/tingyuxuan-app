import { useEffect, useRef, useCallback } from "react";
import { useAppStore } from "../../shared/stores/appStore";
import type { PipelineEvent, UserAction, StructuredError } from "../../shared/lib/types";
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

/** Format duration as M:SS */
function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** 根据状态计算浮动条容器的 className */
function barContainerClass(
  state: string,
  mode: string,
  aiResult: string | null,
): string {
  if (state === "error") return "w-[400px] min-h-[64px]";
  if (state === "done" && mode === "ai_assistant" && aiResult)
    return "w-[420px] h-[360px] flex-col";
  return "w-[400px] h-[56px]";
}

/** 根据状态计算浮动条的样式 */
function barStyleClass(state: string): string {
  switch (state) {
    case "recording":
      return "bg-blue-700/95 border-2 border-blue-400/70 animate-recording-pulse";
    case "processing":
      return "bg-indigo-600/95 border-2 border-indigo-400/60";
    case "done":
      return "bg-emerald-700/95 border-2 border-emerald-400/60";
    case "cancelled":
      return "bg-red-800/95 border-2 border-red-500/60";
    case "error":
      return "bg-red-900/95 border-2 border-red-500/70 animate-error-shake";
    default:
      return "bg-gray-900/90 border border-gray-700/50";
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

  const durationTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const prevStateRef = useRef(recordingState);

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
      durationTimerRef.current = setInterval(() => {
        useAppStore.setState((s) => ({
          recordingDuration: s.recordingDuration + 1,
        }));
      }, 1000);
    } else {
      if (durationTimerRef.current) {
        clearInterval(durationTimerRef.current);
        durationTimerRef.current = null;
      }
    }
    return () => {
      if (durationTimerRef.current) clearInterval(durationTimerRef.current);
    };
  }, [recordingState]);

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
            case "RecordingStarted":
              setLogSession(data.session_id);
              log.info(`session started: mode=${data.mode}`);
              store.setSessionId(data.session_id);
              store.setRecordingState("recording");
              break;
            case "VolumeUpdate":
              store.setVolumeLevels(data.levels);
              break;
            case "RecordingStopped":
              // Don't change state yet; processing is next
              break;
            case "ProcessingStarted":
              store.setRecordingState("processing");
              break;
            case "ProcessingComplete":
              log.info("processing complete");
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

        // Shortcut actions from global shortcuts
        const u2 = await listen("shortcut-action", async (event) => {
          const action = event.payload as string;
          const { invoke } = await import("@tauri-apps/api/core");
          const currentState = useAppStore.getState().recordingState;
          log.debug(`shortcut: ${action}`, { currentState });

          switch (action) {
            case "cancel":
              if (currentState === "recording" || currentState === "processing") {
                store.setRecordingState("cancelled");
                invoke("cancel_recording").catch(() => {});
              }
              break;
            case "stop":
              if (currentState === "recording") {
                store.setRecordingState("processing");
                invoke("stop_recording").catch(() => {});
              }
              break;
            case "dictate":
            case "translate":
            case "ai_assistant":
              if (currentState === "idle") {
                store.setRecordingMode(action as "dictate" | "translate" | "ai_assistant");
                // 乐观更新：立即显示录音 UI，不等待 RecordingStarted 事件
                store.setRecordingState("recording");
                invoke<string>("start_recording", { mode: action })
                  .then((sessionId) => {
                    store.setSessionId(sessionId);
                  })
                  .catch((errStr: string) => {
                    try {
                      const se = JSON.parse(errStr) as StructuredError;
                      store.setError(se.message, se.user_action);
                    } catch {
                      store.setError(errStr, "Retry");
                    }
                  });
              }
              break;
          }
        });
        if (cleaned) { u2(); return; }
        unlisteners.push(u2);
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
    setRecordingState("processing");
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

  return (
    <div className="floating-bar-window h-screen w-screen flex items-center justify-center">
      <div
        className={`
          flex items-center rounded-2xl shadow-2xl
          backdrop-blur-md animate-bar-enter
          ${barStyleClass(recordingState)}
          ${barContainerClass(recordingState, recordingMode, aiResult)}
        `}
      >
        {/* Recording state */}
        {recordingState === "recording" && (
          <>
            {/* Cancel button */}
            <button
              onClick={handleCancel}
              className="flex-none w-10 h-10 flex items-center justify-center
                         text-white/70 hover:text-red-300 hover:bg-red-500/20
                         rounded-xl ml-2 transition-colors"
              title="取消 (Esc)"
              aria-label="取消录制"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>

            {/* Waveform + info */}
            <div className="flex-1 flex flex-col items-center px-2">
              <div className="w-full h-8">
                <Waveform levels={volumeLevels} isActive={true} />
              </div>
              <div className="flex items-center gap-2 text-[10px] text-white/70">
                <span className="flex items-center gap-1">
                  <span className="w-1.5 h-1.5 rounded-full bg-red-400 animate-pulse" />
                  {MODE_LABELS[recordingMode] || recordingMode}
                </span>
                <span className="text-white/30">|</span>
                <span className="tabular-nums">{formatDuration(recordingDuration)}</span>
              </div>
            </div>

            {/* Confirm button */}
            <button
              onClick={handleConfirm}
              className="flex-none w-10 h-10 flex items-center justify-center
                         text-white/70 hover:text-green-300 hover:bg-green-500/20
                         rounded-xl mr-2 transition-colors"
              title="完成"
              aria-label="停止录制"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            </button>
          </>
        )}

        {/* Processing state — 脉冲进度条 */}
        {recordingState === "processing" && (
          <div className="flex-1 flex flex-col items-center justify-center gap-2 px-4">
            <div className="flex items-center gap-3">
              <div className="w-5 h-5 border-2 border-white/60 border-t-transparent rounded-full animate-spin" />
              <span className="text-white/90 text-sm">处理中...</span>
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
            <svg className="w-4 h-4 text-white/80" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
            <span className="text-white/80 text-sm">已取消</span>
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
