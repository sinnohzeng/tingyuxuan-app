use crate::error::AudioError;
use crate::stt::streaming::{AudioChunk, STREAMING_CHANNEL_CAPACITY};
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Number of samples per RMS computation window (~30ms at 16 kHz = 480 samples).
const RMS_WINDOW_SAMPLES: usize = 480;
/// Maximum number of recent RMS levels retained for the waveform UI.
const MAX_RMS_LEVELS: usize = 200;

/// Mutable interior state shared between the main thread and the cpal audio
/// callback thread.
struct RecorderInner {
    is_recording: bool,
    sample_count: u64,
    /// Accumulator for the current RMS window.
    rms_accumulator: Vec<f32>,
    /// Recent RMS levels for waveform rendering in the UI.
    rms_levels: VecDeque<f32>,
    /// Channel to send audio chunks to the STT pipeline.
    /// `try_send` 用于背压控制，满时丢帧（语音 STT 容忍少量丢帧）。
    audio_tx: Option<mpsc::Sender<AudioChunk>>,
}

/// Audio recorder that captures microphone input and outputs 16 kHz / 16-bit /
/// mono PCM frames via a channel for real-time streaming STT.
///
/// Thread safety is provided through `Arc<Mutex<RecorderInner>>` so the cpal
/// input callback can safely write into the shared state.
///
/// # Mock mode
///
/// When the environment variable `TINGYUXUAN_MOCK_AUDIO=1` is set, the recorder
/// generates synthetic silence instead of opening a real microphone. This is
/// useful for CI environments and automated testing.
pub struct AudioRecorder {
    inner: Arc<Mutex<RecorderInner>>,
    /// Holds the cpal stream while recording.  Dropping this stops the stream.
    stream: Option<Stream>,
    /// Handle for the mock audio thread (joined on stop/cancel to prevent leaks).
    mock_thread: Option<std::thread::JoinHandle<()>>,
    /// True when running in mock mode (no real microphone).
    mock_mode: bool,
}

impl AudioRecorder {
    /// Creates a new `AudioRecorder`.
    ///
    /// In mock mode (`TINGYUXUAN_MOCK_AUDIO=1`) no audio device initialisation
    /// is performed.  Otherwise the default cpal host is probed to make sure
    /// there is at least one input device available.
    pub fn new() -> Result<Self, AudioError> {
        let mock_mode = std::env::var("TINGYUXUAN_MOCK_AUDIO")
            .map(|v| v == "1")
            .unwrap_or(false);

        if !mock_mode {
            // Probe for an input device early so callers get a clear error.
            let host = cpal::default_host();
            let _device = host
                .default_input_device()
                .ok_or(AudioError::NoInputDevice)?;
        }

        let inner = RecorderInner {
            is_recording: false,
            sample_count: 0,
            rms_accumulator: Vec::with_capacity(RMS_WINDOW_SAMPLES),
            rms_levels: VecDeque::with_capacity(MAX_RMS_LEVELS),
            audio_tx: None,
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            stream: None,
            mock_thread: None,
            mock_mode,
        })
    }

    /// Starts recording and returns a channel receiver for PCM audio chunks.
    ///
    /// Audio frames are 16 kHz mono PCM16. The channel is bounded
    /// (`STREAMING_CHANNEL_CAPACITY` frames ≈ 1s at 20ms/frame); when full,
    /// frames are silently dropped (STT tolerates minor frame loss).
    ///
    /// Dropping the returned `Receiver` or calling [`stop`] ends the stream.
    pub fn start(&mut self) -> Result<mpsc::Receiver<AudioChunk>, AudioError> {
        {
            let inner = self
                .inner
                .lock()
                .expect("RecorderInner: lock poisoned in start() check");
            if inner.is_recording {
                return Err(AudioError::AlreadyRecording);
            }
        }

        let (tx, rx) = mpsc::channel(STREAMING_CHANNEL_CAPACITY);

        {
            let mut inner = self
                .inner
                .lock()
                .expect("RecorderInner: lock poisoned in start() setup");
            inner.is_recording = true;
            inner.audio_tx = Some(tx);
            inner.sample_count = 0;
            inner.rms_accumulator.clear();
            inner.rms_levels.clear();
        }

        if self.mock_mode {
            self.start_mock_stream()?;
        } else {
            self.start_real_stream()?;
        }

        Ok(rx)
    }

    /// Stops recording.
    ///
    /// Drops the audio channel sender, signaling the receiver that no more
    /// frames will arrive. Joins the mock thread if running in mock mode.
    pub fn stop(&mut self) -> Result<(), AudioError> {
        // Drop the stream first so the callback stops producing frames.
        self.stream.take();

        {
            let mut inner = self
                .inner
                .lock()
                .expect("RecorderInner: lock poisoned in stop()");
            if !inner.is_recording {
                return Err(AudioError::NotRecording);
            }
            inner.is_recording = false;
            // Drop the sender → receiver sees channel closed.
            inner.audio_tx.take();
        }

        // Join mock thread to prevent leak.
        if let Some(handle) = self.mock_thread.take() {
            let _ = handle.join();
        }

        Ok(())
    }

    /// Cancels the current recording (same as stop).
    pub fn cancel(&mut self) -> Result<(), AudioError> {
        self.stop()
    }

    /// Returns a copy of the recent RMS volume levels for waveform rendering.
    ///
    /// Each value is in the range `[0.0, 1.0]` where 0 is silence and 1.0 is
    /// full scale.
    pub fn get_volume_levels(&self) -> Vec<f32> {
        let inner = self
            .inner
            .lock()
            .expect("RecorderInner: lock poisoned in get_volume_levels()");
        inner.rms_levels.iter().copied().collect()
    }

    /// Returns `true` if the recorder is currently recording.
    pub fn is_recording(&self) -> bool {
        let inner = self
            .inner
            .lock()
            .expect("RecorderInner: lock poisoned in is_recording()");
        inner.is_recording
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// Starts recording from the real default input device using cpal.
    fn start_real_stream(&mut self) -> Result<(), AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;

        let supported = device.supported_input_configs().map_err(|e| {
            AudioError::StreamError(format!("Failed to query input configs: {}", e))
        })?;

        let config = Self::select_input_config(supported)?;
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let inner = Arc::clone(&self.inner);
        let channels = stream_config.channels as usize;
        let device_sample_rate = stream_config.sample_rate;

        let err_callback = |err: cpal::StreamError| {
            tracing::error!("cpal stream error: {}", err);
        };

        let stream = match sample_format {
            cpal::SampleFormat::I16 => device
                .build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        Self::process_input_i16(data, channels, device_sample_rate, &inner);
                    },
                    err_callback,
                    None,
                )
                .map_err(|e| {
                    AudioError::StreamError(format!("Failed to build i16 input stream: {}", e))
                })?,
            cpal::SampleFormat::F32 => device
                .build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        Self::process_input_f32(data, channels, device_sample_rate, &inner);
                    },
                    err_callback,
                    None,
                )
                .map_err(|e| {
                    AudioError::StreamError(format!("Failed to build f32 input stream: {}", e))
                })?,
            cpal::SampleFormat::U16 => device
                .build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        Self::process_input_u16(data, channels, device_sample_rate, &inner);
                    },
                    err_callback,
                    None,
                )
                .map_err(|e| {
                    AudioError::StreamError(format!("Failed to build u16 input stream: {}", e))
                })?,
            _ => {
                return Err(AudioError::StreamError(format!(
                    "Unsupported sample format: {:?}",
                    sample_format
                )));
            }
        };

        stream
            .play()
            .map_err(|e| AudioError::StreamError(format!("Failed to start audio stream: {}", e)))?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Selects the best supported input configuration.
    fn select_input_config(
        supported: cpal::SupportedInputConfigs,
    ) -> Result<cpal::SupportedStreamConfig, AudioError> {
        let mut configs: Vec<cpal::SupportedStreamConfigRange> = supported.collect();
        if configs.is_empty() {
            return Err(AudioError::StreamError(
                "No supported input configurations found".to_string(),
            ));
        }

        // Prefer a config that includes 16 kHz.
        let target_rate = 16_000u32;
        for cfg in &configs {
            if cfg.min_sample_rate() <= target_rate && cfg.max_sample_rate() >= target_rate {
                return Ok((*cfg).with_sample_rate(target_rate));
            }
        }

        // Fall back to the config closest to 16 kHz (prefer integer multiples like 48k/32k).
        configs.sort_by_key(|c| {
            let rate = c.max_sample_rate() as i64;
            (rate - target_rate as i64).unsigned_abs()
        });
        let best = configs.first().unwrap();
        // Use the max sample rate but cap at 48 kHz (beyond that wastes CPU resampling).
        let rate = best.max_sample_rate().min(48_000);
        let clamped = rate.clamp(best.min_sample_rate(), best.max_sample_rate());
        Ok((*best).with_sample_rate(clamped))
    }

    /// Starts a mock stream that sends silence frames on a background thread.
    fn start_mock_stream(&mut self) -> Result<(), AudioError> {
        let inner = Arc::clone(&self.inner);

        let handle = std::thread::spawn(move || {
            // Generate ~30ms chunks of silence at 16 kHz until stopped.
            loop {
                {
                    let mut guard = match inner.lock() {
                        Ok(g) => g,
                        Err(poisoned) => {
                            tracing::error!("Audio buffer lock poisoned, recovering");
                            poisoned.into_inner()
                        }
                    };
                    if !guard.is_recording {
                        break;
                    }

                    // 通过 channel 发送音频帧（silence 无状态，直接构造）。
                    if let Some(ref tx) = guard.audio_tx {
                        let chunk = AudioChunk {
                            samples: vec![0i16; RMS_WINDOW_SAMPLES],
                        };
                        let _ = tx.try_send(chunk);
                    }
                    guard.sample_count += RMS_WINDOW_SAMPLES as u64;

                    // Push a zero-level RMS entry.
                    if guard.rms_levels.len() >= MAX_RMS_LEVELS {
                        guard.rms_levels.pop_front();
                    }
                    guard.rms_levels.push_back(0.0);
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        });

        self.mock_thread = Some(handle);
        Ok(())
    }

    // ------------------------------------------------------------------
    // Input processing helpers (called from the cpal audio callback)
    // ------------------------------------------------------------------

    fn process_input_i16(
        data: &[i16],
        channels: usize,
        device_sample_rate: u32,
        inner: &Arc<Mutex<RecorderInner>>,
    ) {
        // Convert to f32 samples, take only channel 0 for mono.
        let mono_f32: Vec<f32> = data
            .chunks(channels)
            .map(|frame| frame[0] as f32 / i16::MAX as f32)
            .collect();
        Self::process_mono_f32(&mono_f32, device_sample_rate, inner);
    }

    fn process_input_f32(
        data: &[f32],
        channels: usize,
        device_sample_rate: u32,
        inner: &Arc<Mutex<RecorderInner>>,
    ) {
        let mono_f32: Cow<'_, [f32]> = if channels == 1 {
            Cow::Borrowed(data)
        } else {
            Cow::Owned(data.chunks(channels).map(|frame| frame[0]).collect())
        };
        Self::process_mono_f32(&mono_f32, device_sample_rate, inner);
    }

    fn process_input_u16(
        data: &[u16],
        channels: usize,
        device_sample_rate: u32,
        inner: &Arc<Mutex<RecorderInner>>,
    ) {
        let mono_f32: Vec<f32> = data
            .chunks(channels)
            .map(|frame| (frame[0] as f32 / u16::MAX as f32) * 2.0 - 1.0)
            .collect();
        Self::process_mono_f32(&mono_f32, device_sample_rate, inner);
    }

    /// Core processing: accepts mono f32 samples in [-1.0, 1.0], resamples to
    /// 16 kHz when necessary, sends PCM frames via channel, and computes RMS.
    fn process_mono_f32(
        samples: &[f32],
        device_sample_rate: u32,
        inner: &Arc<Mutex<RecorderInner>>,
    ) {
        // Simple nearest-neighbour resample when the device rate differs from
        // 16 kHz.  This is good enough for voice dictation.
        let resampled: Cow<'_, [f32]> = if device_sample_rate == 16_000 {
            Cow::Borrowed(samples)
        } else {
            let ratio = 16_000.0 / device_sample_rate as f64;
            let out_len = (samples.len() as f64 * ratio).ceil() as usize;
            Cow::Owned(
                (0..out_len)
                    .map(|i| {
                        let src_idx = ((i as f64) / ratio).min((samples.len() - 1) as f64) as usize;
                        samples[src_idx]
                    })
                    .collect(),
            )
        };

        // Convert to i16 for STT.
        let pcm: Vec<i16> = resampled
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect();

        let mut guard = match inner.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::error!("Audio buffer lock poisoned, recovering");
                poisoned.into_inner()
            }
        };

        if !guard.is_recording {
            return;
        }

        // 通过 channel 发送音频帧（背压控制：满时丢帧）。
        let pcm_len = pcm.len();
        if let Some(ref tx) = guard.audio_tx {
            let chunk = AudioChunk { samples: pcm };
            let _ = tx.try_send(chunk);
        }
        guard.sample_count += pcm_len as u64;

        // RMS computation over ~30ms windows.
        for &sample in resampled.as_ref() {
            guard.rms_accumulator.push(sample);
            if guard.rms_accumulator.len() >= RMS_WINDOW_SAMPLES {
                let sum_sq: f32 = guard.rms_accumulator.iter().map(|s| s * s).sum();
                let rms = (sum_sq / guard.rms_accumulator.len() as f32).sqrt();
                guard.rms_accumulator.clear();

                if guard.rms_levels.len() >= MAX_RMS_LEVELS {
                    guard.rms_levels.pop_front();
                }
                guard.rms_levels.push_back(rms);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a recorder in mock mode for testing.
    fn mock_recorder() -> AudioRecorder {
        // SAFETY: Tests run with TINGYUXUAN_MOCK_AUDIO already set; no concurrent reads race.
        unsafe { std::env::set_var("TINGYUXUAN_MOCK_AUDIO", "1") };
        AudioRecorder::new().expect("mock recorder should succeed")
    }

    #[test]
    fn test_new_mock_mode() {
        // SAFETY: Tests run with TINGYUXUAN_MOCK_AUDIO already set; no concurrent reads race.
        unsafe { std::env::set_var("TINGYUXUAN_MOCK_AUDIO", "1") };
        let recorder = AudioRecorder::new();
        assert!(recorder.is_ok());
        assert!(recorder.unwrap().mock_mode);
    }

    #[test]
    fn test_not_recording_initially() {
        let recorder = mock_recorder();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_stop_without_start_returns_error() {
        let mut recorder = mock_recorder();
        let result = recorder.stop();
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_without_start_returns_error() {
        let mut recorder = mock_recorder();
        let result = recorder.cancel();
        assert!(result.is_err());
    }

    #[test]
    fn test_start_and_stop() {
        let mut recorder = mock_recorder();

        let rx = recorder.start().expect("start should succeed");
        assert!(recorder.is_recording());

        // Give the mock thread a moment to send some frames.
        std::thread::sleep(std::time::Duration::from_millis(100));

        recorder.stop().expect("stop should succeed");
        assert!(!recorder.is_recording());

        // Channel should be closed — no more frames.
        drop(rx);
    }

    #[test]
    fn test_start_returns_audio_chunks() {
        let mut recorder = mock_recorder();

        let mut rx = recorder.start().expect("start should succeed");

        // Give the mock thread a moment to send some frames.
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Should have received at least one chunk.
        let chunk = rx.try_recv();
        assert!(chunk.is_ok(), "should receive at least one audio chunk");
        assert!(!chunk.unwrap().samples.is_empty());

        recorder.stop().unwrap();
    }

    #[test]
    fn test_double_start_returns_error() {
        let mut recorder = mock_recorder();

        let _rx = recorder.start().unwrap();
        let result = recorder.start();
        assert!(result.is_err());

        // Cleanup.
        let _ = recorder.stop();
    }

    #[test]
    fn test_cancel() {
        let mut recorder = mock_recorder();

        let _rx = recorder.start().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        recorder.cancel().expect("cancel should succeed");
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_get_volume_levels() {
        let mut recorder = mock_recorder();

        let _rx = recorder.start().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));

        let levels = recorder.get_volume_levels();
        // In mock mode the levels should all be 0.0 (silence).
        assert!(!levels.is_empty());
        assert!(levels.iter().all(|&l| l == 0.0));

        let _ = recorder.stop();
    }

    #[test]
    fn test_rms_computation() {
        // Directly test the RMS computation logic via process_mono_f32.
        let (tx, _rx) = mpsc::channel(STREAMING_CHANNEL_CAPACITY);
        let inner = Arc::new(Mutex::new(RecorderInner {
            is_recording: true,
            sample_count: 0,
            rms_accumulator: Vec::new(),
            rms_levels: VecDeque::new(),
            audio_tx: Some(tx),
        }));

        // Create a full RMS window of constant amplitude 0.5.
        let samples = vec![0.5f32; RMS_WINDOW_SAMPLES];
        AudioRecorder::process_mono_f32(&samples, 16_000, &inner);

        let guard = inner.lock().unwrap();
        assert_eq!(guard.rms_levels.len(), 1);
        // RMS of constant 0.5 is 0.5.
        assert!((guard.rms_levels[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_channel_backpressure() {
        // 验证 channel 满时不会 panic，只是丢帧。
        let (tx, _rx) = mpsc::channel(2); // 极小容量
        let inner = Arc::new(Mutex::new(RecorderInner {
            is_recording: true,
            sample_count: 0,
            rms_accumulator: Vec::new(),
            rms_levels: VecDeque::new(),
            audio_tx: Some(tx),
        }));

        // 发送大量帧，不应 panic。
        for _ in 0..100 {
            let samples = vec![0.1f32; 320];
            AudioRecorder::process_mono_f32(&samples, 16_000, &inner);
        }

        // 只要不 panic 就是成功。
        let guard = inner.lock().unwrap();
        assert!(guard.sample_count > 0);
    }
}
