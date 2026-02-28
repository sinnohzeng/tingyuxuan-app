import { create } from "zustand";
import type {
  RecordingState,
  RecordingMode,
  UserAction,
  AppConfig,
} from "../lib/types";

interface AppStore {
  // Recording state
  recordingState: RecordingState;
  recordingMode: RecordingMode;
  volumeLevels: number[];
  recordingDuration: number; // seconds
  sessionId: string | null;

  // Error state
  errorMessage: string | null;
  errorAction: UserAction | null;
  rawTranscript: string | null;

  // Network
  isOnline: boolean;

  // AI assistant result
  aiResult: string | null;

  // Config
  config: AppConfig | null;

  // Actions
  setRecordingState: (state: RecordingState) => void;
  setRecordingMode: (mode: RecordingMode) => void;
  setVolumeLevels: (levels: number[]) => void;
  setRecordingDuration: (duration: number) => void;
  setSessionId: (id: string | null) => void;
  setError: (
    message: string,
    action: UserAction,
    rawText?: string | null
  ) => void;
  clearError: () => void;
  setIsOnline: (online: boolean) => void;
  setAiResult: (text: string | null) => void;
  setConfig: (config: AppConfig) => void;
  reset: () => void;
}

export const useAppStore = create<AppStore>((set) => ({
  // Initial state
  recordingState: "idle",
  recordingMode: "dictate",
  volumeLevels: [],
  recordingDuration: 0,
  sessionId: null,
  errorMessage: null,
  errorAction: null,
  rawTranscript: null,
  aiResult: null,
  isOnline: true,
  config: null,

  // Actions
  setRecordingState: (recordingState) => set({ recordingState }),
  setRecordingMode: (recordingMode) => set({ recordingMode }),
  setVolumeLevels: (volumeLevels) => set({ volumeLevels }),
  setRecordingDuration: (recordingDuration) => set({ recordingDuration }),
  setSessionId: (sessionId) => set({ sessionId }),
  setError: (message, action, rawText) =>
    set({
      recordingState: "error",
      errorMessage: message,
      errorAction: action,
      rawTranscript: rawText ?? null,
    }),
  clearError: () =>
    set({ recordingState: "idle", errorMessage: null, errorAction: null, rawTranscript: null }),
  setAiResult: (aiResult) => set({ aiResult }),
  setIsOnline: (isOnline) => set({ isOnline }),
  setConfig: (config) => set({ config }),
  // isOnline is global network state, intentionally not reset per session
  reset: () =>
    set({
      recordingState: "idle",
      recordingMode: "dictate",
      volumeLevels: [],
      recordingDuration: 0,
      sessionId: null,
      errorMessage: null,
      errorAction: null,
      rawTranscript: null,
      aiResult: null,
    }),
}));
