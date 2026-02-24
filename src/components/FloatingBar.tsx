import { useEffect, useRef, useCallback } from "react";
import { useAppStore } from "../stores/appStore";
import Waveform from "./Waveform";
import ErrorPanel from "./ErrorPanel";

/** Mode display labels */
const MODE_LABELS: Record<string, string> = {
  dictate: "听写",
  translate: "翻译模式",
  ai_assistant: "AI 助手",
  edit: "语音编辑",
};

export default function FloatingBar() {
  const {
    recordingState,
    recordingMode,
    volumeLevels,
    recordingDuration,
    errorMessage,
    errorAction,
    rawTranscript,
    setRecordingState,
    reset,
  } = useAppStore();

  const durationTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Format duration as M:SS
  const formatDuration = (seconds: number): string => {
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  };

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

  // Auto-hide after done/cancelled
  useEffect(() => {
    if (recordingState === "done" || recordingState === "cancelled") {
      hideTimerRef.current = setTimeout(() => {
        reset();
      }, 1500);
    }
    return () => {
      if (hideTimerRef.current) clearTimeout(hideTimerRef.current);
    };
  }, [recordingState, reset]);

  // Listen for Tauri events (if running in Tauri context)
  useEffect(() => {
    let unlisteners: Array<() => void> = [];

    async function setupListeners() {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const store = useAppStore.getState();

        const u1 = await listen("pipeline-event", (event) => {
          const data = event.payload as Record<string, unknown>;
          switch (data.type) {
            case "RecordingStarted":
              store.setSessionId(data.session_id as string);
              store.setRecordingState("recording");
              break;
            case "VolumeUpdate":
              store.setVolumeLevels(data.levels as number[]);
              break;
            case "RecordingStopped":
              // Don't change state yet; processing is next
              break;
            case "TranscriptionStarted":
            case "ProcessingStarted":
              store.setRecordingState("processing");
              break;
            case "TranscriptionComplete":
              // STT done, keep processing state
              break;
            case "ProcessingComplete":
              store.setRecordingState("done");
              break;
            case "Error":
              store.setError(
                data.message as string,
                data.user_action as
                  | "RetryOrQueue"
                  | "InsertRawOrRetry"
                  | "CheckApiKey"
                  | "WaitAndRetry"
                  | "CheckMicrophone",
                data.raw_text as string | null
              );
              break;
          }
        });
        unlisteners.push(u1);
      } catch {
        // Not running in Tauri - that's fine for dev mode
      }
    }

    setupListeners();
    return () => unlisteners.forEach((fn) => fn());
  }, []);

  const handleCancel = useCallback(() => {
    setRecordingState("cancelled");
    // TODO: invoke('cancel_recording')
  }, [setRecordingState]);

  const handleConfirm = useCallback(() => {
    setRecordingState("processing");
    // TODO: invoke('stop_recording')
  }, [setRecordingState]);

  const handleRetry = useCallback(() => {
    setRecordingState("processing");
    // TODO: invoke retry
  }, [setRecordingState]);

  const handleInsertRaw = useCallback(() => {
    // TODO: invoke text injection with rawTranscript
    reset();
  }, [reset]);

  const handleDismiss = useCallback(() => {
    reset();
  }, [reset]);

  const handleOpenSettings = useCallback(() => {
    // TODO: open settings window
    reset();
  }, [reset]);

  // Don't render anything when idle
  if (recordingState === "idle") {
    return null;
  }

  return (
    <div className="floating-bar-window h-screen w-screen flex items-center justify-center">
      <div
        className={`
          flex items-center rounded-2xl shadow-2xl border
          transition-all duration-200
          ${
            recordingState === "cancelled"
              ? "bg-red-900/90 border-red-700/50"
              : "bg-gray-900/90 border-gray-700/50"
          }
          backdrop-blur-md
          ${recordingState === "error" ? "w-[400px] min-h-[64px]" : "w-[400px] h-[56px]"}
        `}
      >
        {/* Recording state */}
        {recordingState === "recording" && (
          <>
            {/* Cancel button */}
            <button
              onClick={handleCancel}
              className="flex-none w-10 h-10 flex items-center justify-center
                         text-gray-400 hover:text-red-400 hover:bg-red-900/30
                         rounded-xl ml-2 transition-colors"
              title="取消 (Esc)"
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
              <div className="flex items-center gap-2 text-[10px] text-gray-400">
                <span>{MODE_LABELS[recordingMode] || recordingMode}</span>
                <span className="text-gray-600">|</span>
                <span className="tabular-nums">{formatDuration(recordingDuration)}</span>
              </div>
            </div>

            {/* Confirm button */}
            <button
              onClick={handleConfirm}
              className="flex-none w-10 h-10 flex items-center justify-center
                         text-gray-400 hover:text-green-400 hover:bg-green-900/30
                         rounded-xl mr-2 transition-colors"
              title="完成"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            </button>
          </>
        )}

        {/* Processing state */}
        {recordingState === "processing" && (
          <div className="flex-1 flex items-center justify-center gap-3 px-4">
            <div className="w-5 h-5 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
            <span className="text-gray-300 text-sm">处理中...</span>
          </div>
        )}

        {/* Done state */}
        {recordingState === "done" && (
          <div className="flex-1 flex items-center justify-center gap-2 px-4">
            <svg className="w-5 h-5 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
            <span className="text-green-400 text-sm">完成</span>
          </div>
        )}

        {/* Cancelled state */}
        {recordingState === "cancelled" && (
          <div className="flex-1 flex items-center justify-center gap-2 px-4">
            <span className="text-red-400 text-sm">已取消</span>
          </div>
        )}

        {/* Error state */}
        {recordingState === "error" && errorMessage && errorAction && (
          <ErrorPanel
            message={errorMessage}
            action={errorAction}
            rawTranscript={rawTranscript}
            onRetry={handleRetry}
            onInsertRaw={handleInsertRaw}
            onDismiss={handleDismiss}
            onOpenSettings={handleOpenSettings}
          />
        )}
      </div>
    </div>
  );
}
