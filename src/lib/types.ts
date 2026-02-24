/** Recording state machine */
export type RecordingState =
  | "idle"
  | "recording"
  | "processing"
  | "done"
  | "cancelled"
  | "error";

/** Recording/processing mode */
export type RecordingMode = "dictate" | "translate" | "ai_assistant" | "edit";

/** Pipeline event from Rust backend */
export type PipelineEvent =
  | { type: "RecordingStarted"; session_id: string; mode: string }
  | { type: "VolumeUpdate"; levels: number[] }
  | { type: "RecordingStopped"; duration_ms: number }
  | { type: "TranscriptionStarted" }
  | { type: "TranscriptionComplete"; raw_text: string }
  | { type: "ProcessingStarted" }
  | { type: "ProcessingComplete"; processed_text: string }
  | {
      type: "Error";
      message: string;
      user_action: UserAction;
      raw_text: string | null;
    }
  | { type: "NetworkStatusChanged"; online: boolean }
  | { type: "QueuedForLater"; session_id: string }
  | { type: "RecordingCancelled" };

/** User action to show on error */
export type UserAction =
  | "RetryOrQueue"
  | "InsertRawOrRetry"
  | "CheckApiKey"
  | "WaitAndRetry"
  | "CheckMicrophone";

/** Floating bar position */
export type FloatingBarPosition = "bottom_center" | "follow_cursor" | "fixed";

/** STT Provider type */
export type STTProviderType = "whisper" | "dashscope_asr" | "custom";

/** LLM Provider type */
export type LLMProviderType = "openai" | "dashscope" | "volcengine" | "custom";

/** App configuration (mirrors Rust AppConfig) */
export interface AppConfig {
  general: {
    auto_launch: boolean;
    sound_feedback: boolean;
    floating_bar_position: FloatingBarPosition;
  };
  shortcuts: {
    dictate: string;
    translate: string;
    ai_assistant: string;
    cancel: string;
  };
  language: {
    primary: string;
    translation_target: string;
    variant: string | null;
  };
  stt: {
    provider: STTProviderType;
    api_key_ref: string;
    base_url: string | null;
    model: string | null;
  };
  llm: {
    provider: LLMProviderType;
    api_key_ref: string;
    base_url: string | null;
    model: string;
  };
  cache: {
    audio_retention_hours: number;
    failed_retention_days: number;
    max_cache_size_mb: number;
  };
}

/** Transcript history record */
export interface TranscriptRecord {
  id: string;
  timestamp: string;
  mode: string;
  raw_text: string | null;
  processed_text: string | null;
  audio_path: string | null;
  status: string;
  app_context: string | null;
  duration_ms: number | null;
  language: string | null;
  error_message: string | null;
}

/** Provider preset for quick setup */
export interface ProviderPreset {
  name: string;
  llm_base_url: string;
  llm_models: string[];
  stt_provider: STTProviderType;
  stt_base_url: string | null;
  stt_model: string | null;
}
