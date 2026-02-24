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
    set({ errorMessage: null, errorAction: null, rawTranscript: null }),
  setConfig: (config) => set({ config }),
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
    }),
}));
