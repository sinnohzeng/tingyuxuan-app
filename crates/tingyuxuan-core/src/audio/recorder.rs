use crate::audio::wav_writer::WavFileWriter;
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Number of samples per RMS computation window (~30ms at 16 kHz = 480 samples).
const RMS_WINDOW_SAMPLES: usize = 480;
/// Interval between disk flushes in milliseconds.
const FLUSH_INTERVAL_MS: u64 = 500;
/// Maximum number of recent RMS levels retained for the waveform UI.
const MAX_RMS_LEVELS: usize = 200;

/// Mutable interior state shared between the main thread and the cpal audio
/// callback thread.
struct RecorderInner {
    is_recording: bool,
    wav_writer: Option<WavFileWriter>,
    audio_path: PathBuf,
    sample_count: u64,
    /// Accumulator for the current RMS window.
    rms_accumulator: Vec<f32>,
    /// Recent RMS levels for waveform rendering in the UI.
    rms_levels: Vec<f32>,
    /// Timestamp of the last flush to disk.
    last_flush: Instant,
}

/// Audio recorder that captures microphone input and writes 16 kHz / 16-bit /
/// mono WAV files.
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
            wav_writer: None,
            audio_path: PathBuf::new(),
            sample_count: 0,
            rms_accumulator: Vec::with_capacity(RMS_WINDOW_SAMPLES),
            rms_levels: Vec::with_capacity(MAX_RMS_LEVELS),
            last_flush: Instant::now(),
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            stream: None,
            mock_mode,
        })
    }

    /// Starts recording audio to a WAV file.
    ///
    /// The WAV file is created at
    /// `{cache_dir}/{ISO_timestamp}_{mode}_{session_id}.wav`.
    ///
    /// Returns the path to the WAV file being written.
    pub fn start(
        &mut self,
        session_id: &str,
        mode: &str,
        cache_dir: &Path,
    ) -> Result<PathBuf, AudioError> {
        {
            let inner = self.inner.lock().unwrap();
            if inner.is_recording {
                return Err(AudioError::AlreadyRecording);
            }
        }

        // Build the output path.
        std::fs::create_dir_all(cache_dir)?;
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let filename = format!("{}_{}_{}_.wav", timestamp, mode, session_id);
        let audio_path = cache_dir.join(&filename);

        let wav_writer = WavFileWriter::new(&audio_path)?;

        {
            let mut inner = self.inner.lock().unwrap();
            inner.is_recording = true;
            inner.wav_writer = Some(wav_writer);
            inner.audio_path = audio_path.clone();
            inner.sample_count = 0;
            inner.rms_accumulator.clear();
            inner.rms_levels.clear();
            inner.last_flush = Instant::now();
        }

        if self.mock_mode {
            self.start_mock_stream()?;
        } else {
            self.start_real_stream()?;
        }

        Ok(audio_path)
    }

    /// Stops recording and finalizes the WAV file.
    ///
    /// Returns the path to the completed WAV file.
    pub fn stop(&mut self) -> Result<PathBuf, AudioError> {
        // Drop the stream first so the callback stops writing.
        self.stream.take();

        let mut inner = self.inner.lock().unwrap();
        if !inner.is_recording {
            return Err(AudioError::NotRecording);
        }
        inner.is_recording = false;

        let wav_writer = inner
            .wav_writer
            .take()
            .ok_or(AudioError::NotRecording)?;
        wav_writer.finalize()?;

        Ok(inner.audio_path.clone())
    }

    /// Cancels the current recording and deletes the WAV file.
    pub fn cancel(&mut self) -> Result<(), AudioError> {
        // Drop the stream.
        self.stream.take();

        let mut inner = self.inner.lock().unwrap();
        if !inner.is_recording {
            return Err(AudioError::NotRecording);
        }
        inner.is_recording = false;

        // Drop the writer without finalizing so the file is not valid.
        let _wav_writer = inner.wav_writer.take();

        // Best-effort delete.
        let path = inner.audio_path.clone();
        drop(inner);
        let _ = std::fs::remove_file(&path);

        Ok(())
    }

    /// Returns a copy of the recent RMS volume levels for waveform rendering.
    ///
    /// Each value is in the range `[0.0, 1.0]` where 0 is silence and 1.0 is
    /// full scale.
    pub fn get_volume_levels(&self) -> Vec<f32> {
        let inner = self.inner.lock().unwrap();
        inner.rms_levels.clone()
    }

    /// Returns `true` if the recorder is currently recording.
    pub fn is_recording(&self) -> bool {
        let inner = self.inner.lock().unwrap();
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

        // Request a config close to what we want: mono, 16 kHz.
        // cpal will give us whatever the device supports and we resample /
        // convert in the callback.
        let supported = device.supported_input_configs().map_err(|e| {
            AudioError::StreamError(format!("Failed to query input configs: {}", e))
        })?;

        // Try to find a config that supports 16 kHz; fall back to the default.
        let config = Self::select_input_config(supported)?;
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let inner = Arc::clone(&self.inner);
        let channels = stream_config.channels as usize;
        let device_sample_rate = stream_config.sample_rate.0;

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

        stream.play().map_err(|e| {
            AudioError::StreamError(format!("Failed to start audio stream: {}", e))
        })?;

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
        let target_rate = cpal::SampleRate(16_000);
        for cfg in &configs {
            if cfg.min_sample_rate() <= target_rate && cfg.max_sample_rate() >= target_rate {
                return Ok(cfg.clone().with_sample_rate(target_rate));
            }
        }

        // Fall back to the config with the highest max sample rate.
        configs.sort_by_key(|c| c.max_sample_rate().0);
        let best = configs.last().unwrap();
        Ok(best.clone().with_max_sample_rate())
    }

    /// Starts a mock stream that writes silence on a background thread.
    fn start_mock_stream(&mut self) -> Result<(), AudioError> {
        let inner = Arc::clone(&self.inner);

        std::thread::spawn(move || {
            // Generate ~30ms chunks of silence at 16 kHz until stopped.
            let chunk = vec![0i16; RMS_WINDOW_SAMPLES];
            loop {
                {
                    let mut guard = inner.lock().unwrap();
                    if !guard.is_recording {
                        break;
                    }
                    if let Some(ref mut writer) = guard.wav_writer {
                        if writer.write_samples(&chunk).is_err() {
                            break;
                        }
                        guard.sample_count += chunk.len() as u64;
                    }
                    // Push a zero-level RMS entry.
                    if guard.rms_levels.len() >= MAX_RMS_LEVELS {
                        guard.rms_levels.remove(0);
                    }
                    guard.rms_levels.push(0.0);

                    // Periodic flush.
                    if guard.last_flush.elapsed().as_millis() >= FLUSH_INTERVAL_MS as u128 {
                        if let Some(ref mut writer) = guard.wav_writer {
                            let _ = writer.flush();
                        }
                        guard.last_flush = Instant::now();
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        });

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
        let mono_f32: Vec<f32> = data.chunks(channels).map(|frame| frame[0]).collect();
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
    /// 16 kHz when necessary, writes to WAV, computes RMS, and flushes
    /// periodically.
    fn process_mono_f32(
        samples: &[f32],
        device_sample_rate: u32,
        inner: &Arc<Mutex<RecorderInner>>,
    ) {
        // Simple nearest-neighbour resample when the device rate differs from
        // 16 kHz.  This is good enough for voice dictation.
        let resampled: Vec<f32> = if device_sample_rate == 16_000 {
            samples.to_vec()
        } else {
            let ratio = 16_000.0 / device_sample_rate as f64;
            let out_len = (samples.len() as f64 * ratio).ceil() as usize;
            (0..out_len)
                .map(|i| {
                    let src_idx = ((i as f64) / ratio).min((samples.len() - 1) as f64) as usize;
                    samples[src_idx]
                })
                .collect()
        };

        // Convert to i16 for WAV writing.
        let pcm: Vec<i16> = resampled
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect();

        let mut guard = match inner.lock() {
            Ok(g) => g,
            Err(_) => return, // poisoned lock – nothing we can do
        };

        if !guard.is_recording {
            return;
        }

        // Write PCM to WAV.
        if let Some(ref mut writer) = guard.wav_writer {
            if writer.write_samples(&pcm).is_err() {
                tracing::error!("Failed to write audio samples to WAV");
                return;
            }
        }
        guard.sample_count += pcm.len() as u64;

        // RMS computation over ~30ms windows.
        for &sample in &resampled {
            guard.rms_accumulator.push(sample);
            if guard.rms_accumulator.len() >= RMS_WINDOW_SAMPLES {
                let sum_sq: f32 = guard.rms_accumulator.iter().map(|s| s * s).sum();
                let rms = (sum_sq / guard.rms_accumulator.len() as f32).sqrt();
                guard.rms_accumulator.clear();

                if guard.rms_levels.len() >= MAX_RMS_LEVELS {
                    guard.rms_levels.remove(0);
                }
                guard.rms_levels.push(rms);
            }
        }

        // Periodic flush.
        if guard.last_flush.elapsed().as_millis() >= FLUSH_INTERVAL_MS as u128 {
            if let Some(ref mut writer) = guard.wav_writer {
                let _ = writer.flush();
            }
            guard.last_flush = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Helper: create a recorder in mock mode for testing.
    fn mock_recorder() -> AudioRecorder {
        std::env::set_var("TINGYUXUAN_MOCK_AUDIO", "1");
        AudioRecorder::new().expect("mock recorder should succeed")
    }

    #[test]
    fn test_new_mock_mode() {
        std::env::set_var("TINGYUXUAN_MOCK_AUDIO", "1");
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
        let dir = tempdir().unwrap();

        let path = recorder
            .start("sess1", "dictate", dir.path())
            .expect("start should succeed");

        assert!(recorder.is_recording());
        assert!(path.to_string_lossy().contains("dictate"));
        assert!(path.to_string_lossy().contains("sess1"));

        // Give the mock thread a moment to write some data.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let final_path = recorder.stop().expect("stop should succeed");
        assert_eq!(path, final_path);
        assert!(!recorder.is_recording());

        // The WAV file should exist and be valid.
        assert!(final_path.exists());
        let reader = hound::WavReader::open(&final_path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
    }

    #[test]
    fn test_double_start_returns_error() {
        let mut recorder = mock_recorder();
        let dir = tempdir().unwrap();

        recorder.start("s1", "dictate", dir.path()).unwrap();
        let result = recorder.start("s2", "translate", dir.path());
        assert!(result.is_err());

        // Cleanup.
        let _ = recorder.stop();
    }

    #[test]
    fn test_cancel_deletes_file() {
        let mut recorder = mock_recorder();
        let dir = tempdir().unwrap();

        let path = recorder.start("s1", "dictate", dir.path()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        recorder.cancel().expect("cancel should succeed");
        assert!(!recorder.is_recording());
        assert!(!path.exists(), "WAV file should be deleted after cancel");
    }

    #[test]
    fn test_get_volume_levels() {
        let mut recorder = mock_recorder();
        let dir = tempdir().unwrap();

        recorder.start("s1", "dictate", dir.path()).unwrap();
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
        let inner = Arc::new(Mutex::new(RecorderInner {
            is_recording: true,
            wav_writer: None, // skip WAV writing for this test
            audio_path: PathBuf::new(),
            sample_count: 0,
            rms_accumulator: Vec::new(),
            rms_levels: Vec::new(),
            last_flush: Instant::now(),
        }));

        // Create a full RMS window of constant amplitude 0.5.
        let samples = vec![0.5f32; RMS_WINDOW_SAMPLES];
        AudioRecorder::process_mono_f32(&samples, 16_000, &inner);

        let guard = inner.lock().unwrap();
        assert_eq!(guard.rms_levels.len(), 1);
        // RMS of constant 0.5 is 0.5.
        assert!((guard.rms_levels[0] - 0.5).abs() < 0.01);
    }
}
