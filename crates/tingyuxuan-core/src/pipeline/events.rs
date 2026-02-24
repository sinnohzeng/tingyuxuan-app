use serde::Serialize;

use crate::error::UserAction;

/// Events emitted by the pipeline as it progresses through each stage.
///
/// The frontend subscribes to these via a `broadcast::Receiver` and updates
/// the floating-bar UI accordingly.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PipelineEvent {
    /// Recording has started for a new session.
    RecordingStarted {
        session_id: String,
        mode: String,
    },
    /// Real-time microphone volume levels (for waveform visualization).
    VolumeUpdate {
        levels: Vec<f32>,
    },
    /// Recording stopped; includes the total duration.
    RecordingStopped {
        duration_ms: u64,
    },
    /// STT transcription has been submitted.
    TranscriptionStarted,
    /// STT transcription completed successfully.
    TranscriptionComplete {
        raw_text: String,
    },
    /// LLM processing has started.
    ProcessingStarted,
    /// LLM processing completed successfully.
    ProcessingComplete {
        processed_text: String,
    },
    /// An error occurred at some pipeline stage.
    Error {
        message: String,
        user_action: UserAction,
        /// When LLM fails but STT succeeded, this carries the raw transcript
        /// so the user can choose to insert it directly.
        raw_text: Option<String>,
    },
    /// Network reachability changed.
    NetworkStatusChanged {
        online: bool,
    },
    /// The recording was saved to the offline queue for later processing.
    QueuedForLater {
        session_id: String,
    },
    /// The current recording was cancelled by the user.
    RecordingCancelled,
}
