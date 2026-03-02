//! Recorder Actor — runs AudioRecorder on a dedicated OS thread.
//!
//! cpal's `Stream` type may be `!Send` on some platforms, and `AudioRecorder`
//! uses `&mut self` methods.  Wrapping it in `Arc<Mutex<_>>` inside Tauri's
//! async command handlers would either deadlock or violate Send bounds.
//!
//! Instead we spawn a dedicated OS thread with its own single-threaded tokio
//! runtime.  The main application communicates via an mpsc command channel.
//! Volume levels are pushed as `PipelineEvent::VolumeUpdate` every ~33ms
//! while recording (no polling from the frontend).

use std::time::Duration;

use tokio::sync::{broadcast, mpsc, oneshot};

use tingyuxuan_core::audio::encoder::AudioBuffer;
use tingyuxuan_core::audio::recorder::AudioRecorder;
use tingyuxuan_core::error::AudioError;
use tingyuxuan_core::pipeline::events::PipelineEvent;

// ---------------------------------------------------------------------------
// Commands sent to the recorder actor
// ---------------------------------------------------------------------------

enum RecorderCommand {
    Start {
        reply: oneshot::Sender<Result<(), AudioError>>,
    },
    Stop {
        reply: oneshot::Sender<Result<AudioBuffer, AudioError>>,
    },
    Cancel {
        reply: oneshot::Sender<Result<(), AudioError>>,
    },
    IsRecording {
        reply: oneshot::Sender<bool>,
    },
}

// ---------------------------------------------------------------------------
// Handle (Send + Sync) that the rest of the app holds
// ---------------------------------------------------------------------------

/// A thread-safe, `Send + Sync` handle to the recorder actor.
///
/// All methods are async and return once the actor has processed the command.
pub struct RecorderHandle {
    cmd_tx: mpsc::Sender<RecorderCommand>,
}

impl RecorderHandle {
    /// Spawn the recorder actor on a dedicated OS thread.
    ///
    /// Volume updates are pushed to `event_tx` every ~33ms while recording.
    pub fn spawn(event_tx: broadcast::Sender<PipelineEvent>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<RecorderCommand>(32);

        std::thread::Builder::new()
            .name("recorder-actor".into())
            .spawn(move || {
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to create tokio runtime for recorder actor");

                    rt.block_on(run_actor(cmd_rx, event_tx));
                })) {
                    tracing::error!("Recorder actor panicked: {e:?}");
                }
            })
            .expect("failed to spawn recorder actor thread");

        RecorderHandle { cmd_tx }
    }

    /// Start recording.
    pub async fn start(&self) -> Result<(), AudioError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(RecorderCommand::Start { reply: tx })
            .await
            .map_err(|_| AudioError::StreamError("Recorder actor is gone".into()))?;
        rx.await
            .map_err(|_| AudioError::StreamError("Recorder reply channel dropped".into()))?
    }

    /// Stop recording and return the accumulated audio buffer.
    pub async fn stop(&self) -> Result<AudioBuffer, AudioError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(RecorderCommand::Stop { reply: tx })
            .await
            .map_err(|_| AudioError::StreamError("Recorder actor is gone".into()))?;
        rx.await
            .map_err(|_| AudioError::StreamError("Recorder reply channel dropped".into()))?
    }

    /// Cancel the current recording.
    pub async fn cancel(&self) -> Result<(), AudioError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(RecorderCommand::Cancel { reply: tx })
            .await
            .map_err(|_| AudioError::StreamError("Recorder actor is gone".into()))?;
        rx.await
            .map_err(|_| AudioError::StreamError("Recorder reply channel dropped".into()))?
    }

    /// Check whether the recorder is currently recording.
    pub async fn is_recording(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(RecorderCommand::IsRecording { reply: tx })
            .await
            .is_err()
        {
            return false;
        }
        rx.await.unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Actor event loop
// ---------------------------------------------------------------------------

async fn run_actor(
    mut cmd_rx: mpsc::Receiver<RecorderCommand>,
    event_tx: broadcast::Sender<PipelineEvent>,
) {
    let mut recorder = match AudioRecorder::new() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to create AudioRecorder: {e}");
            // Keep draining commands so senders don't hang.
            drain_with_error(&mut cmd_rx, &format!("Audio not available: {e}")).await;
            return;
        }
    };

    // Volume push timer — ticks every ~33ms (≈30 fps).
    let mut volume_interval = tokio::time::interval(Duration::from_millis(33));
    volume_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(cmd) => handle_command(cmd, &mut recorder),
                    None => break, // All senders dropped — shut down.
                }
            }
            _ = volume_interval.tick() => {
                if recorder.is_recording() {
                    let levels = recorder.get_volume_levels();
                    let _ = event_tx.send(PipelineEvent::VolumeUpdate { levels });
                }
            }
        }
    }

    tracing::info!("Recorder actor shutting down");
}

fn handle_command(cmd: RecorderCommand, recorder: &mut AudioRecorder) {
    match cmd {
        RecorderCommand::Start { reply } => {
            tracing::debug!("Recorder command: Start");
            let result = recorder.start();
            if reply.send(result).is_err() {
                tracing::warn!("Reply channel dropped (caller timed out?)");
            }
        }
        RecorderCommand::Stop { reply } => {
            tracing::debug!("Recorder command: Stop");
            let result = recorder.stop();
            if reply.send(result).is_err() {
                tracing::warn!("Reply channel dropped (caller timed out?)");
            }
        }
        RecorderCommand::Cancel { reply } => {
            tracing::debug!("Recorder command: Cancel");
            let result = recorder.cancel();
            if reply.send(result).is_err() {
                tracing::warn!("Reply channel dropped (caller timed out?)");
            }
        }
        RecorderCommand::IsRecording { reply } => {
            if reply.send(recorder.is_recording()).is_err() {
                tracing::warn!("Reply channel dropped (caller timed out?)");
            }
        }
    }
}

/// Drain all incoming commands with an error (used when the recorder
/// failed to initialise).
async fn drain_with_error(cmd_rx: &mut mpsc::Receiver<RecorderCommand>, error_msg: &str) {
    let err = || AudioError::StreamError(format!("Audio not available: {error_msg}"));
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            RecorderCommand::Start { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            RecorderCommand::Stop { reply } => {
                let _ = reply.send(Err(err()));
            }
            RecorderCommand::Cancel { reply } => {
                let _ = reply.send(Err(err()));
            }
            RecorderCommand::IsRecording { reply } => {
                let _ = reply.send(false);
            }
        }
    }
}
