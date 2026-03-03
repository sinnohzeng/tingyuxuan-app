use crate::audio::devices;
use crate::audio::encoder::{AudioBuffer, MAX_SAMPLES};
use crate::error::AudioError;
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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
    /// PCM 缓冲区：累积所有录音采样，录音结束后编码为目标格式。
    buffer: Vec<i16>,
    /// 是否因超过最大采样数而自动停止。
    auto_stopped: bool,
}

/// Audio recorder that captures microphone input and accumulates 16 kHz / 16-bit
/// mono PCM samples into an internal buffer.
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
    /// 用户选择的输入设备 ID。`None` = 系统默认。
    device_id: Option<String>,
}

impl AudioRecorder {
    /// Creates a new `AudioRecorder`.
    ///
    /// `device_id` 指定目标麦克风（`DeviceId.to_string()`）；`None` 使用系统默认设备。
    /// Mock 模式（`TINGYUXUAN_MOCK_AUDIO=1`）下忽略 device_id，不初始化音频硬件。
    pub fn new(device_id: Option<&str>) -> Result<Self, AudioError> {
        let mock_mode = std::env::var("TINGYUXUAN_MOCK_AUDIO")
            .map(|v| v == "1")
            .unwrap_or(false);

        if !mock_mode {
            // Probe for an input device early so callers get a clear error.
            let _device = devices::resolve_input_device(device_id)?;
        }

        let inner = RecorderInner {
            is_recording: false,
            sample_count: 0,
            rms_accumulator: Vec::with_capacity(RMS_WINDOW_SAMPLES),
            rms_levels: VecDeque::with_capacity(MAX_RMS_LEVELS),
            buffer: Vec::new(),
            auto_stopped: false,
        };

        if mock_mode {
            tracing::info!("AudioRecorder initialized (mock mode)");
        } else {
            tracing::info!(device_id, "AudioRecorder initialized");
        }

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            stream: None,
            mock_thread: None,
            mock_mode,
            device_id: device_id.map(String::from),
        })
    }

    /// 探测麦克风是否可用（静态方法，无需创建 AudioRecorder 实例）。
    ///
    /// 用于权限检测：`default_input_device()` None → NoInputDevice；
    /// `supported_input_configs()` 失败 → PermissionDenied。
    /// Mock 模式下始终返回 Ok。
    pub fn probe_microphone() -> Result<(), AudioError> {
        let mock = std::env::var("TINGYUXUAN_MOCK_AUDIO")
            .map(|v| v == "1")
            .unwrap_or(false);
        if mock {
            return Ok(());
        }
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;
        device
            .supported_input_configs()
            .map_err(|_| AudioError::PermissionDenied)?
            .next()
            .ok_or(AudioError::NoInputDevice)?;
        Ok(())
    }

    /// Starts recording. PCM samples are accumulated in an internal buffer.
    ///
    /// Call [`stop`] to end recording and retrieve the accumulated `AudioBuffer`.
    pub fn start(&mut self) -> Result<(), AudioError> {
        {
            let mut inner = self
                .inner
                .lock()
                .expect("RecorderInner: lock poisoned in start()");
            if inner.is_recording {
                return Err(AudioError::AlreadyRecording);
            }
            inner.is_recording = true;
            inner.sample_count = 0;
            inner.rms_accumulator.clear();
            inner.rms_levels.clear();
            inner.buffer.clear();
            inner.auto_stopped = false;
        }

        tracing::debug!(mock = self.mock_mode, "Starting audio capture");
        let result = if self.mock_mode {
            self.start_mock_stream()
        } else {
            self.start_real_stream()
        };

        // 流启动失败时重置 is_recording，避免卡在虚假录音状态
        if result.is_err() {
            let mut inner = self
                .inner
                .lock()
                .expect("RecorderInner: lock poisoned in start() rollback");
            inner.is_recording = false;
        }

        result
    }

    /// Stops recording and returns the accumulated audio buffer.
    ///
    /// The buffer contains all PCM samples captured during recording.
    pub fn stop(&mut self) -> Result<AudioBuffer, AudioError> {
        // Drop the stream first so the callback stops producing frames.
        self.stream.take();

        let (samples, sample_count) = {
            let mut inner = self
                .inner
                .lock()
                .expect("RecorderInner: lock poisoned in stop()");
            if !(inner.is_recording || (inner.auto_stopped && !inner.buffer.is_empty())) {
                return Err(AudioError::NotRecording);
            }
            if inner.auto_stopped {
                tracing::info!("Stopping recorder after max-duration auto-stop");
            }
            inner.is_recording = false;
            inner.auto_stopped = false;
            (std::mem::take(&mut inner.buffer), inner.sample_count)
        };

        // Join mock thread to prevent leak.
        if let Some(handle) = self.mock_thread.take() {
            let _ = handle.join();
        }

        tracing::info!(samples = sample_count, "Audio recording stopped");

        let mut audio_buffer = AudioBuffer::new(16_000, 1);
        audio_buffer.push_samples(&samples);
        Ok(audio_buffer)
    }

    /// Cancels the current recording, discarding all captured audio.
    pub fn cancel(&mut self) -> Result<(), AudioError> {
        // Drop the stream.
        self.stream.take();

        {
            let mut inner = self
                .inner
                .lock()
                .expect("RecorderInner: lock poisoned in cancel()");
            if !inner.is_recording {
                return Err(AudioError::NotRecording);
            }
            inner.is_recording = false;
            inner.buffer.clear();
            inner.buffer.shrink_to_fit();
        }

        if let Some(handle) = self.mock_thread.take() {
            let _ = handle.join();
        }

        tracing::info!("Audio recording cancelled");
        Ok(())
    }

    /// Returns a copy of the recent RMS volume levels for waveform rendering.
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

    /// Returns `true` if recording was auto-stopped due to max duration.
    pub fn was_auto_stopped(&self) -> bool {
        let inner = self
            .inner
            .lock()
            .expect("RecorderInner: lock poisoned in was_auto_stopped()");
        inner.auto_stopped
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// Starts recording from the configured input device using cpal.
    fn start_real_stream(&mut self) -> Result<(), AudioError> {
        let device = devices::resolve_input_device(self.device_id.as_deref())?;

        // 优先使用设备默认配置（WASAPI 最可靠的路径），fallback 到手动选择
        let config = match device.default_input_config() {
            Ok(c) if Self::format_priority(c.sample_format()) > 0 => c,
            _ => {
                let supported = device.supported_input_configs().map_err(|e| {
                    AudioError::StreamError(format!("Failed to query input configs: {}", e))
                })?;
                Self::select_input_config(supported)?
            }
        };
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        #[allow(deprecated)] // cpal 0.17.3 deprecated name() in favor of description()
        let device_name = device.name().unwrap_or_else(|_| "unknown".into());
        tracing::info!(
            device = %device_name,
            sample_rate = stream_config.sample_rate,
            channels = stream_config.channels,
            format = ?sample_format,
            "Audio device selected"
        );

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
            cpal::SampleFormat::U8 => device
                .build_input_stream(
                    &stream_config,
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        Self::process_input_u8(data, channels, device_sample_rate, &inner);
                    },
                    err_callback,
                    None,
                )
                .map_err(|e| {
                    AudioError::StreamError(format!("Failed to build u8 input stream: {}", e))
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

    /// 采样格式优先级评分：I16 > F32 > U16 > U8 > 其他
    fn format_priority(format: cpal::SampleFormat) -> u8 {
        match format {
            cpal::SampleFormat::I16 => 4,
            cpal::SampleFormat::F32 => 3,
            cpal::SampleFormat::U16 => 2,
            cpal::SampleFormat::U8 => 1,
            _ => 0,
        }
    }

    /// Selects the best supported input configuration.
    ///
    /// 优先级：支持的采样格式（I16 > F32 > U16 > U8）→ 包含 16kHz → 采样率最接近 16kHz。
    fn select_input_config(
        supported: cpal::SupportedInputConfigs,
    ) -> Result<cpal::SupportedStreamConfig, AudioError> {
        let mut configs: Vec<cpal::SupportedStreamConfigRange> = supported.collect();
        if configs.is_empty() {
            return Err(AudioError::StreamError(
                "No supported input configurations found".to_string(),
            ));
        }

        let target_rate = 16_000u32;

        // 按格式优先级排序（高优先在前），同格式按采样率接近 16kHz 排序
        configs.sort_by(|a, b| {
            let fa = Self::format_priority(a.sample_format());
            let fb = Self::format_priority(b.sample_format());
            fb.cmp(&fa).then_with(|| {
                let da = (a.max_sample_rate() as i64 - target_rate as i64).unsigned_abs();
                let db = (b.max_sample_rate() as i64 - target_rate as i64).unsigned_abs();
                da.cmp(&db)
            })
        });

        // 优先选包含 16kHz 的高优先级格式
        for cfg in &configs {
            if Self::format_priority(cfg.sample_format()) == 0 {
                continue; // 跳过不支持的格式
            }
            if cfg.min_sample_rate() <= target_rate && cfg.max_sample_rate() >= target_rate {
                return Ok((*cfg).with_sample_rate(target_rate));
            }
        }

        // 回退：选第一个支持的格式（已按优先级排序）
        let best = configs
            .iter()
            .find(|c| Self::format_priority(c.sample_format()) > 0)
            .ok_or_else(|| {
                AudioError::StreamError("No supported sample format found".to_string())
            })?;
        let rate = best.max_sample_rate().min(48_000);
        let clamped = rate.clamp(best.min_sample_rate(), best.max_sample_rate());
        Ok((*best).with_sample_rate(clamped))
    }

    /// Starts a mock stream that generates silence on a background thread.
    fn start_mock_stream(&mut self) -> Result<(), AudioError> {
        let inner = Arc::clone(&self.inner);

        let handle = std::thread::spawn(move || {
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

                    // 写入 silence 到缓冲区。
                    let silence = vec![0i16; RMS_WINDOW_SAMPLES];
                    guard.buffer.extend_from_slice(&silence);
                    guard.sample_count += RMS_WINDOW_SAMPLES as u64;

                    // 录音时长上限检查。
                    if guard.buffer.len() >= MAX_SAMPLES {
                        guard.is_recording = false;
                        guard.auto_stopped = true;
                        tracing::warn!("Recording auto-stopped: max duration reached");
                        break;
                    }

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

    fn process_input_u8(
        data: &[u8],
        channels: usize,
        device_sample_rate: u32,
        inner: &Arc<Mutex<RecorderInner>>,
    ) {
        // U8 音频：128 为静音中心，范围 0-255 → 映射到 [-1.0, 1.0]
        let mono_f32: Vec<f32> = data
            .chunks(channels)
            .map(|frame| (frame[0] as f32 - 128.0) / 128.0)
            .collect();
        Self::process_mono_f32(&mono_f32, device_sample_rate, inner);
    }

    /// Core processing: accepts mono f32 samples in [-1.0, 1.0], resamples to
    /// 16 kHz when necessary, writes PCM to buffer, and computes RMS.
    fn process_mono_f32(
        samples: &[f32],
        device_sample_rate: u32,
        inner: &Arc<Mutex<RecorderInner>>,
    ) {
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

        // Convert to i16.
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

        // 写入 PCM 到缓冲区。
        let pcm_len = pcm.len();
        guard.buffer.extend_from_slice(&pcm);
        guard.sample_count += pcm_len as u64;

        // 录音时长上限检查。
        if guard.buffer.len() >= MAX_SAMPLES {
            guard.is_recording = false;
            guard.auto_stopped = true;
            tracing::warn!("Recording auto-stopped: max duration reached");
        }

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
    use serial_test::serial;

    /// Helper: create a recorder in mock mode for testing.
    fn mock_recorder() -> AudioRecorder {
        temp_env::with_var("TINGYUXUAN_MOCK_AUDIO", Some("1"), || {
            AudioRecorder::new(None).expect("mock recorder should succeed")
        })
    }

    #[test]
    #[serial]
    fn test_new_mock_mode() {
        temp_env::with_var("TINGYUXUAN_MOCK_AUDIO", Some("1"), || {
            let recorder = AudioRecorder::new(None);
            assert!(recorder.is_ok());
            assert!(recorder.unwrap().mock_mode);
        });
    }

    #[test]
    #[serial]
    fn test_new_with_device_id_mock() {
        temp_env::with_var("TINGYUXUAN_MOCK_AUDIO", Some("1"), || {
            let recorder = AudioRecorder::new(Some("nonexistent-device"));
            assert!(recorder.is_ok());
            assert_eq!(
                recorder.unwrap().device_id.as_deref(),
                Some("nonexistent-device")
            );
        });
    }

    #[test]
    #[serial]
    fn test_not_recording_initially() {
        let recorder = mock_recorder();
        assert!(!recorder.is_recording());
    }

    #[test]
    #[serial]
    fn test_stop_without_start_returns_error() {
        let mut recorder = mock_recorder();
        let result = recorder.stop();
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_cancel_without_start_returns_error() {
        let mut recorder = mock_recorder();
        let result = recorder.cancel();
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_stop_after_auto_stopped_returns_buffer() {
        let mut recorder = mock_recorder();

        {
            let mut inner = recorder.inner.lock().unwrap();
            inner.is_recording = false;
            inner.auto_stopped = true;
            inner.sample_count = 3;
            inner.buffer = vec![1, 2, 3];
        }

        let buffer = recorder.stop().expect("stop should return buffered audio");
        assert_eq!(buffer.len(), 3);
        assert!(!recorder.was_auto_stopped());
    }

    #[test]
    #[serial]
    fn test_start_and_stop() {
        let mut recorder = mock_recorder();

        recorder.start().expect("start should succeed");
        assert!(recorder.is_recording());

        // Give the mock thread a moment to generate some frames.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let buffer = recorder.stop().expect("stop should succeed");
        assert!(!recorder.is_recording());
        assert!(!buffer.is_empty());
        assert!(buffer.duration_ms() > 0);
    }

    #[test]
    #[serial]
    fn test_start_returns_buffer_with_samples() {
        let mut recorder = mock_recorder();

        recorder.start().expect("start should succeed");

        // Give the mock thread a moment to generate some frames.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let buffer = recorder.stop().unwrap();
        assert!(!buffer.is_empty());
    }

    #[test]
    #[serial]
    fn test_double_start_returns_error() {
        let mut recorder = mock_recorder();

        recorder.start().unwrap();
        let result = recorder.start();
        assert!(result.is_err());

        // Cleanup.
        let _ = recorder.stop();
    }

    #[test]
    #[serial]
    fn test_cancel() {
        let mut recorder = mock_recorder();

        recorder.start().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        recorder.cancel().expect("cancel should succeed");
        assert!(!recorder.is_recording());
    }

    #[test]
    #[serial]
    fn test_get_volume_levels() {
        let mut recorder = mock_recorder();

        recorder.start().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));

        let levels = recorder.get_volume_levels();
        // In mock mode the levels should all be 0.0 (silence).
        assert!(!levels.is_empty());
        assert!(levels.iter().all(|&l| l == 0.0));

        let _ = recorder.stop();
    }

    #[test]
    fn test_rms_computation() {
        let inner = Arc::new(Mutex::new(RecorderInner {
            is_recording: true,
            sample_count: 0,
            rms_accumulator: Vec::new(),
            rms_levels: VecDeque::new(),
            buffer: Vec::new(),
            auto_stopped: false,
        }));

        // Create a full RMS window of constant amplitude 0.5.
        let samples = vec![0.5f32; RMS_WINDOW_SAMPLES];
        AudioRecorder::process_mono_f32(&samples, 16_000, &inner);

        let guard = inner.lock().unwrap();
        assert_eq!(guard.rms_levels.len(), 1);
        // RMS of constant 0.5 is 0.5.
        assert!((guard.rms_levels[0] - 0.5).abs() < 0.01);
        // Buffer should have PCM samples.
        assert_eq!(guard.buffer.len(), RMS_WINDOW_SAMPLES);
    }

    #[test]
    fn test_buffer_accumulation() {
        let inner = Arc::new(Mutex::new(RecorderInner {
            is_recording: true,
            sample_count: 0,
            rms_accumulator: Vec::new(),
            rms_levels: VecDeque::new(),
            buffer: Vec::new(),
            auto_stopped: false,
        }));

        // 发送多个批次的采样。
        for _ in 0..10 {
            let samples = vec![0.1f32; 320];
            AudioRecorder::process_mono_f32(&samples, 16_000, &inner);
        }

        let guard = inner.lock().unwrap();
        assert_eq!(guard.buffer.len(), 3200);
        assert_eq!(guard.sample_count, 3200);
    }
}
