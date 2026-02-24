# Audio 模块

## 模块职责

Audio 模块负责麦克风音频采集和录音文件缓存管理。通过 cpal 库录制 16 kHz / 16-bit / 单声道 WAV 文件，并提供带有 JSON sidecar 元数据的文件缓存生命周期管理。

---

## 核心类型定义

### AudioRecorder

```rust
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
```

**内部共享状态：**

```rust
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
```

**关键常量：**

```rust
/// Number of samples per RMS computation window (~30ms at 16 kHz = 480 samples).
const RMS_WINDOW_SAMPLES: usize = 480;
/// Interval between disk flushes in milliseconds.
const FLUSH_INTERVAL_MS: u64 = 500;
/// Maximum number of recent RMS levels retained for the waveform UI.
const MAX_RMS_LEVELS: usize = 200;
```

### AudioCache

```rust
/// Manages the lifecycle of audio recording cache files.
///
/// Audio files are stored in `{data_dir}/cache/audio/` alongside sidecar JSON
/// metadata files (`*.wav.json`).
pub struct AudioCache {
    cache_dir: PathBuf,
}
```

### AudioMetadata

```rust
/// Sidecar metadata for an audio recording file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Recording mode (e.g. "dictate", "translate", "ai_assistant").
    pub mode: String,
    /// Current status: "recording", "completed", "failed", "pending".
    pub status: String,
    /// Duration of the audio in milliseconds (0 while still recording).
    pub duration_ms: u64,
    /// ISO 8601 timestamp when the recording was created.
    pub created_at: String,
    /// ISO 8601 timestamp of the last status update.
    pub updated_at: String,
}
```

### WavFileWriter

```rust
/// WAV file writer that wraps `hound::WavWriter` with a buffered output.
///
/// Produces 16 kHz, 16-bit, mono PCM WAV files suitable for speech-to-text
/// services.
pub struct WavFileWriter {
    writer: WavWriter<BufWriter<File>>,
    sample_count: u64,
}
```

### AudioError

```rust
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("No audio input device found")]
    NoInputDevice,
    #[error("Microphone permission denied")]
    PermissionDenied,
    #[error("Microphone is in use by another application")]
    DeviceBusy,
    #[error("Audio stream error: {0}")]
    StreamError(String),
    #[error("WAV write error: {0}")]
    WavWriteError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Not recording")]
    NotRecording,
    #[error("Already recording")]
    AlreadyRecording,
}
```

---

## 公开 API

### AudioRecorder

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new() -> Result<Self, AudioError>` | 创建录音器实例。Mock 模式下跳过设备初始化，正常模式下探测默认输入设备 |
| `start()` | `fn start(&mut self, session_id: &str, mode: &str, cache_dir: &Path) -> Result<PathBuf, AudioError>` | 开始录音，返回 WAV 文件路径。文件名格式：`{ISO_timestamp}_{mode}_{session_id}.wav` |
| `stop()` | `fn stop(&mut self) -> Result<PathBuf, AudioError>` | 停止录音并 finalize WAV 文件头，返回完成的文件路径 |
| `cancel()` | `fn cancel(&mut self) -> Result<(), AudioError>` | 取消录音并删除 WAV 文件（best-effort） |
| `get_volume_levels()` | `fn get_volume_levels(&self) -> Vec<f32>` | 获取最近的 RMS 音量级别（`[0.0, 1.0]` 范围），用于波形可视化 |
| `is_recording()` | `fn is_recording(&self) -> bool` | 当前是否正在录音 |

### AudioCache

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new() -> Result<Self, AudioError>` | 使用 `AppConfig::data_dir()` 创建缓存实例 |
| `with_dir()` | `fn with_dir(cache_dir: PathBuf) -> Result<Self, AudioError>` | 使用指定目录创建缓存实例（主要用于测试） |
| `cache_dir()` | `fn cache_dir(&self) -> &Path` | 返回缓存目录路径 |
| `audio_path()` | `fn audio_path(&self, mode: &str, session_id: &str) -> PathBuf` | 生成新录音文件的路径 |
| `write_metadata()` | `fn write_metadata(&self, audio_path: &Path, mode: &str, status: &str, duration_ms: u64) -> Result<(), AudioError>` | 写入 sidecar JSON 元数据文件 |
| `update_status()` | `fn update_status(&self, audio_path: &Path, status: &str) -> Result<(), AudioError>` | 更新已有 sidecar 的 status 字段 |
| `list_pending()` | `fn list_pending(&self) -> Result<Vec<PathBuf>, AudioError>` | 扫描缓存目录，返回状态为 `"recording"` 或 `"failed"` 的音频文件路径 |
| `cleanup_expired()` | `fn cleanup_expired(&self, max_age_hours: u64) -> Result<u64, AudioError>` | 删除超过指定时间的音频文件及其 sidecar，返回删除数量 |

### WavFileWriter

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(path: &Path) -> Result<Self, AudioError>` | 创建 16 kHz / 16-bit / 单声道 WAV 文件 |
| `write_samples()` | `fn write_samples(&mut self, samples: &[i16]) -> Result<(), AudioError>` | 写入 16-bit PCM 样本 |
| `flush()` | `fn flush(&mut self) -> Result<(), AudioError>` | 刷新内部缓冲区到磁盘（不 finalize 文件头） |
| `finalize()` | `fn finalize(self) -> Result<(), AudioError>` | 写入正确的数据长度并关闭文件（消耗 self） |
| `sample_count()` | `fn sample_count(&self) -> u64` | 已写入的样本总数 |
| `duration_ms()` | `fn duration_ms(&self) -> u64` | 根据样本数和采样率计算时长（毫秒） |

---

## 错误处理策略

- **设备探测阶段：** `new()` 在正常模式下立即探测默认输入设备，缺少设备时返回 `AudioError::NoInputDevice`
- **录音状态守卫：** 重复调用 `start()` 返回 `AlreadyRecording`；在未录音时调用 `stop()` / `cancel()` 返回 `NotRecording`
- **cpal 流错误：** 通过 `AudioError::StreamError(String)` 包装，涵盖配置查询失败、流构建失败、播放失败等场景
- **WAV 写入错误：** 通过 `AudioError::WavWriteError(String)` 包装 hound 库错误
- **IO 错误：** 通过 `AudioError::IoError` 使用 `#[from]` 自动转换 `std::io::Error`
- **Mock 模式：** 设置环境变量 `TINGYUXUAN_MOCK_AUDIO=1` 后跳过所有设备相关操作，生成静音数据
- **文件权限：** Unix 平台上 sidecar 元数据文件设置为 `0600` 权限
- **cancel 容错：** 取消录音时文件删除采用 best-effort（`let _ = std::fs::remove_file()`），不传播删除失败的错误
- **Poisoned lock：** cpal 回调中锁中毒时静默返回，不会 panic

---

## 测试覆盖

共 **22** 个单元测试：

### AudioRecorder 测试（9 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_new_mock_mode` | Mock 模式下初始化成功 |
| `test_not_recording_initially` | 初始状态非录音 |
| `test_stop_without_start_returns_error` | 未开始时 stop 返回错误 |
| `test_cancel_without_start_returns_error` | 未开始时 cancel 返回错误 |
| `test_start_and_stop` | 完整录音-停止流程，验证 WAV 文件有效且采样率正确 |
| `test_double_start_returns_error` | 重复 start 返回 `AlreadyRecording` |
| `test_cancel_deletes_file` | cancel 后 WAV 文件被删除 |
| `test_get_volume_levels` | Mock 模式下音量级别全为 0.0 |
| `test_rms_computation` | 直接验证 RMS 计算逻辑（常量振幅 0.5 的 RMS 值为 0.5） |

### AudioCache 测试（13 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_with_dir_creates_directory` | 创建嵌套目录 |
| `test_audio_path_format` | 文件名包含 mode、session_id 和 .wav 后缀 |
| `test_write_and_read_metadata` | 写入并读取 sidecar JSON |
| `test_update_status` | 更新 status 字段而不改变 mode |
| `test_list_pending` | 正确筛选 `recording` 和 `failed` 状态的文件 |
| `test_list_pending_no_sidecar` | 没有 sidecar 的 WAV 文件被忽略 |
| `test_cleanup_expired` | max_age=0 时所有文件被清理 |
| `test_cleanup_keeps_fresh_files` | 新文件不会被过早清理 |
| `test_metadata_file_permissions` | Unix 下文件权限为 0600 |
| `test_sidecar_path` | sidecar 路径拼接正确 |
| `test_update_status_nonexistent_sidecar` | 不存在的 sidecar 返回错误 |
| WavFileWriter 共 7 个测试 | 创建文件、读写样本、时长计算、flush、空文件、无效路径、多批次写入 |

---

## 已知限制

1. **仅支持 WAV 格式** -- 不支持 Opus 等压缩格式，WAV 文件体积较大（16 kHz 单声道约 32 KB/s）
2. **无 VAD / 静音检测** -- 不会自动跳过静音段，用户需手动控制录音起止
3. **最近邻重采样** -- 当设备采样率非 16 kHz 时使用最近邻插值（nearest-neighbour），精度对语音识别足够但不适用于高保真场景
4. **RMS 级别上限** -- 最多保留 200 个 RMS 级别，超出后从头部移除（`remove(0)` 的 O(n) 复杂度，对 200 的规模可接受）
5. **Sidecar 路径约定** -- 使用 `.wav.json` 后缀（如 `recording.wav.json`），而 recovery 模块使用 `.json` 后缀，两者约定不同
6. **无并发录音** -- 同一 `AudioRecorder` 实例同时只能有一个活跃录音会话
