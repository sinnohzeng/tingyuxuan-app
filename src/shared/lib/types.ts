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
  | { type: "ProcessingStarted" }
  | { type: "ProcessingComplete"; processed_text: string }
  | {
      type: "Error";
      message: string;
      user_action: UserAction;
    }
  | { type: "NetworkStatusChanged"; online: boolean }
  | { type: "RecordingCancelled" };

/** User action to show on error */
export type UserAction =
  | "Retry"
  | "CheckApiKey"
  | "WaitAndRetry"
  | "CheckMicrophone";

/** Floating bar position */
export type FloatingBarPosition = "bottom_center" | "follow_cursor" | "fixed";

/** LLM Provider type */
export type LLMProviderType = "openai" | "dashscope" | "volcengine" | "custom";

/** App configuration (mirrors Rust AppConfig) */
export interface AppConfig {
  general: {
    auto_launch: boolean;
    sound_feedback: boolean;
    floating_bar_position: FloatingBarPosition;
    minimize_to_tray: boolean;
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
  llm: {
    provider: LLMProviderType;
    api_key_ref: string;
    base_url: string | null;
    model: string;
  };
  cache: {
    history_retention_days: number;
  };
  user_dictionary: string[];
}

/** Transcript history record */
export interface TranscriptRecord {
  id: string;
  timestamp: string;
  mode: string;
  raw_text: string | null;
  processed_text: string | null;
  status: string;
  context_json: string | null;
  duration_ms: number | null;
  language: string | null;
  error_message: string | null;
}

/** 设置组件通用的 config 更新函数类型 */
export type ConfigUpdater = (updater: (prev: AppConfig) => AppConfig) => void;

/** 仪表盘聚合统计（镜像 Rust AggregateStats） */
export interface DashboardStats {
  total_sessions: number;
  successful_sessions: number;
  total_duration_ms: number;
  total_char_count: number;
  dictionary_utilization: number; // 0.0 - 1.0
  average_speed_cpm: number; // 字/分钟
  estimated_time_saved_ms: number;
}
