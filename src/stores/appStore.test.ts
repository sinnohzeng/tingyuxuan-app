import { describe, it, expect, beforeEach } from "vitest";
import { useAppStore } from "./appStore";

describe("appStore", () => {
  beforeEach(() => {
    useAppStore.getState().reset();
  });

  it("reset() restores all state to initial values", () => {
    const store = useAppStore.getState();
    store.setRecordingState("recording");
    store.setRecordingMode("translate");
    store.setSessionId("test-session");
    store.setAiResult("some result");

    store.reset();

    const state = useAppStore.getState();
    expect(state.recordingState).toBe("idle");
    expect(state.recordingMode).toBe("dictate");
    expect(state.volumeLevels).toEqual([]);
    expect(state.recordingDuration).toBe(0);
    expect(state.sessionId).toBeNull();
    expect(state.errorMessage).toBeNull();
    expect(state.errorAction).toBeNull();
    expect(state.rawTranscript).toBeNull();
    expect(state.aiResult).toBeNull();
  });

  it("setError() sets error state and recording state to error", () => {
    const store = useAppStore.getState();
    store.setError("Connection failed", "Retry", "raw text");

    const state = useAppStore.getState();
    expect(state.recordingState).toBe("error");
    expect(state.errorMessage).toBe("Connection failed");
    expect(state.errorAction).toBe("Retry");
    expect(state.rawTranscript).toBe("raw text");
  });

  it("clearError() clears error state", () => {
    const store = useAppStore.getState();
    store.setError("error msg", "CheckApiKey");
    store.clearError();

    const state = useAppStore.getState();
    expect(state.errorMessage).toBeNull();
    expect(state.errorAction).toBeNull();
    expect(state.rawTranscript).toBeNull();
  });

  it("setIsOnline() updates network status", () => {
    const store = useAppStore.getState();
    expect(store.isOnline).toBe(true);

    store.setIsOnline(false);
    expect(useAppStore.getState().isOnline).toBe(false);

    store.setIsOnline(true);
    expect(useAppStore.getState().isOnline).toBe(true);
  });

  it("setAiResult() stores AI assistant result", () => {
    const store = useAppStore.getState();
    expect(store.aiResult).toBeNull();

    store.setAiResult("AI response text");
    expect(useAppStore.getState().aiResult).toBe("AI response text");

    store.setAiResult(null);
    expect(useAppStore.getState().aiResult).toBeNull();
  });
});
