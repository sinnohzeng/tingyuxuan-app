//! Telemetry 事件定义。
//!
//! 所有可上报的事件枚举。JSON 序列化时以 `event_type` 作为 tag。

use serde::{Deserialize, Serialize};

/// 可上报的 telemetry 事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum TelemetryEvent {
    /// 录音会话开始。
    #[serde(rename = "session_started")]
    SessionStarted {
        session_id: String,
        mode: String,
        has_context: bool,
    },

    /// 录音会话完成（成功处理）。
    #[serde(rename = "session_completed")]
    SessionCompleted {
        session_id: String,
        recording_ms: u64,
        llm_total_ms: u64,
        result_chars: usize,
        injected: bool,
    },

    /// 录音会话失败。
    #[serde(rename = "session_failed")]
    SessionFailed {
        session_id: String,
        error_code: String,
        stage: String,
        duration_ms: u64,
    },

    /// 录音会话被用户取消。
    #[serde(rename = "session_cancelled")]
    SessionCancelled {
        session_id: String,
        stage: String,
        duration_ms: u64,
    },

    /// 应用启动。
    #[serde(rename = "app_started")]
    AppStarted {
        version: String,
        platform: String,
        has_api_key: bool,
        model: String,
    },

    /// 权限检测结果。
    #[serde(rename = "permission_check")]
    PermissionCheck {
        platform: String,
        microphone: String,
        accessibility: String,
    },

    /// 音频设备信息。
    #[serde(rename = "audio_device_info")]
    AudioDeviceInfo {
        device: String,
        sample_rate: u32,
        channels: u16,
    },

    /// 用户操作（前端埋点）。
    #[serde(rename = "user_action")]
    UserAction {
        action: String,
        context: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_session_started() {
        let event = TelemetryEvent::SessionStarted {
            session_id: "abc".into(),
            mode: "dictate".into(),
            has_context: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event_type\":\"session_started\""));
        assert!(json.contains("\"session_id\":\"abc\""));
    }

    #[test]
    fn test_serialize_user_action() {
        let event = TelemetryEvent::UserAction {
            action: "cancel".into(),
            context: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event_type\":\"user_action\""));
    }
}
