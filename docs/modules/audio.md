# Audio 模块

## 模块职责

Audio 模块负责麦克风音频采集、PCM 缓冲区累积、压缩编码和录音文件缓存管理。
录音器通过 cpal 库采集 16 kHz / 16-bit / 单声道 PCM 数据，累积到内存 `AudioBuffer` 中（而非流式写入文件），
录音结束后由 `AudioBuffer` 优先编码为 MP3（24 kbps），若编码失败自动回退 WAV，再交给多模态 LLM 一步处理。
同时提供带有 JSON sidecar 元数据的文件缓存生命周期管理。

---

## 核心类型定义

### AudioBuffer（编码子模块 `audio/encoder.rs`）

```rust
/// 录音缓冲区：在录音过程中累积 PCM 采样。
pub struct AudioBuffer {
    samples: Vec<i16>,
    sample_rate: u32,
    channels: u16,
}
```

录音器不再通过 channel 流式传输 PCM 到文件，而是将采样数据累积到 `AudioBuffer` 中。录音结束后，由 `AudioBuffer::encode()` 一次性编码为 MP3/WAV。

### EncodedAudio（编码子模块 `audio/encoder.rs`）

```rust
/// 编码后的音频数据。
pub struct EncodedAudio {
    pub data: Vec<u8>,
    pub format: AudioFormat,
    pub duration_ms: u64,
}
```

`EncodedAudio` 是编码后的音频字节序列，支持 `to_base64()` 转为 base64 字符串（用于多模态 LLM API 请求体），以及 `format_str()` 返回格式标识（`"wav"`/`"mp3"`）。

**编码策略：**
- **MP3（默认）**：`shine-rs` 纯 Rust 编码，24 kbps，显著降低上传体积
- **WAV（回退）**：零外部依赖，手动写入 44 字节 RIFF/WAVE 头 + raw PCM16 数据

**关键常量：**

```rust
/// 录音时长上限（秒）。超过后自动停止录音。
pub const MAX_RECORDING_SECONDS: u64 = 300;
/// 录音采样上限（16kHz x 300s = 4_800_000 samples）。
pub const MAX_SAMPLES: usize = 16_000 * MAX_RECORDING_SECONDS as usize;
```

### AudioDeviceInfo（设备子模块 `audio/devices.rs`）

```rust
/// 音频输入设备的可序列化描述。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,         // DeviceId.to_string() — 持久化标识
    pub name: String,       // DeviceDescription — 用户可读名称（UI 显示）
    pub is_default: bool,   // 是否为系统默认输入设备
}
```

设备标识方案详见 [ADR-0009](../architecture/adr/0009-audio-device-selection.md)。

### AudioRecorder

```rust
/// Audio recorder that captures microphone input and accumulates PCM samples
/// in an AudioBuffer (16 kHz / 16-bit / mono).
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
    /// 选中的麦克风设备 ID。None = 系统默认。
    device_id: Option<String>,
}
```

**关键常量：**

```rust
/// Number of samples per RMS computation window (~30ms at 16 kHz = 480 samples).
const RMS_WINDOW_SAMPLES: usize = 480;
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

### 设备枚举与选择（`audio/devices.rs`）

| 函数 | 签名 | 说明 |
|------|------|------|
| `enumerate_input_devices()` | `fn enumerate_input_devices() -> Result<Vec<AudioDeviceInfo>, AudioError>` | 枚举所有可用音频输入设备。Mock 模式返回模拟设备 |
| `resolve_input_device()` | `fn resolve_input_device(device_id: Option<&str>) -> Result<cpal::Device, AudioError>` | 根据持久化的 DeviceId 查找设备。None 或找不到时 fallback 到默认设备 |

### AudioBuffer（编码子模块）

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(sample_rate: u32, channels: u16) -> Self` | 创建空缓冲区 |
| `push_samples()` | `fn push_samples(&mut self, samples: &[i16])` | 追加 PCM 采样 |
| `len()` | `fn len(&self) -> usize` | 当前采样数 |
| `is_empty()` | `fn is_empty(&self) -> bool` | 是否为空 |
| `duration_ms()` | `fn duration_ms(&self) -> u64` | 当前录音时长（毫秒） |
| `exceeds_max_duration()` | `fn exceeds_max_duration(&self) -> bool` | 是否超过 MAX_RECORDING_SECONDS |
| `encode()` | `fn encode(&self, format: AudioFormat) -> Result<EncodedAudio, AudioError>` | 编码为指定格式 |
| `clear()` | `fn clear(&mut self)` | 清空缓冲区，释放内存 |

### EncodedAudio

| 方法 | 签名 | 说明 |
|------|------|------|
| `to_base64()` | `fn to_base64(&self) -> String` | base64 编码（用于 API 请求体） |
| `format_str()` | `fn format_str(&self) -> &'static str` | 返回格式标识（`"wav"` / `"mp3"`） |

### AudioRecorder

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(device_id: Option<&str>) -> Result<Self, AudioError>` | 创建录音器实例。`device_id` 指定麦克风设备（`None` = 系统默认）。Mock 模式下跳过设备初始化 |
| `start()` | `fn start(&mut self, ...) -> Result<(), AudioError>` | 开始录音，PCM 数据累积到内部 AudioBuffer |
| `stop()` | `fn stop(&mut self) -> Result<AudioBuffer, AudioError>` | 停止录音并返回 AudioBuffer（所有权转移） |
| `cancel()` | `fn cancel(&mut self) -> Result<(), AudioError>` | 取消录音并清空缓冲区 |
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

### WavFileWriter（仅用于缓存文件写入）

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(path: &Path) -> Result<Self, AudioError>` | 创建 16 kHz / 16-bit / 单声道 WAV 文件 |
| `write_samples()` | `fn write_samples(&mut self, samples: &[i16]) -> Result<(), AudioError>` | 写入 16-bit PCM 样本 |
| `flush()` | `fn flush(&mut self) -> Result<(), AudioError>` | 刷新内部缓冲区到磁盘（不 finalize 文件头） |
| `finalize()` | `fn finalize(self) -> Result<(), AudioError>` | 写入正确的数据长度并关闭文件（消耗 self） |
| `sample_count()` | `fn sample_count(&self) -> u64` | 已写入的样本总数 |
| `duration_ms()` | `fn duration_ms(&self) -> u64` | 根据样本数和采样率计算时长（毫秒） |

> **注意：** 主处理管线不再使用 `WavFileWriter`。WAV 编码由 `AudioBuffer::encode()` 在内存中零依赖完成。`WavFileWriter` 仅用于缓存文件的持久化场景（如离线队列）。

---

## 错误处理策略

- **设备探测阶段：** `new(device_id)` 在正常模式下通过 `resolve_input_device()` 查找指定设备（或默认设备），缺少设备时返回 `AudioError::NoInputDevice`
- **录音状态守卫：** 重复调用 `start()` 返回 `AlreadyRecording`；在未录音时调用 `stop()` / `cancel()` 返回 `NotRecording`
- **自动截止容错：** 达到 `MAX_RECORDING_SECONDS` 自动停录后，`stop()` 仍可取回缓冲区（避免边界时刻丢音频）
- **cpal 流错误：** 通过 `AudioError::StreamError(String)` 包装，涵盖配置查询失败、流构建失败、播放失败等场景
- **WAV 写入错误：** 通过 `AudioError::WavWriteError(String)` 包装 hound 库错误
- **IO 错误：** 通过 `AudioError::IoError` 使用 `#[from]` 自动转换 `std::io::Error`
- **Mock 模式：** 设置环境变量 `TINGYUXUAN_MOCK_AUDIO=1` 后跳过所有设备相关操作，生成静音数据
- **文件权限：** Unix 平台上 sidecar 元数据文件设置为 `0600` 权限
- **cancel 容错：** 取消录音时文件删除采用 best-effort（`let _ = std::fs::remove_file()`），不传播删除失败的错误
- **Poisoned lock：** cpal 回调中锁中毒时静默返回，不会 panic

---

## 测试覆盖

共 **36** 个单元测试（含编码器 9 个、设备模块 3 个）：

### 设备枚举测试（3 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_enumerate_mock_mode` | Mock 模式返回包含 "Mock Microphone" 的设备列表 |
| `test_mock_device_is_default` | Mock 模式返回的设备 is_default=true |
| `test_audio_device_info_serialization` | AudioDeviceInfo JSON 序列化/反序列化往返一致 |

### AudioBuffer / EncodedAudio 编码器测试（11 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_empty_buffer` | 空缓冲区初始状态 |
| `test_push_samples` | 追加采样后长度正确 |
| `test_duration_ms` | 16000 samples = 1000ms |
| `test_exceeds_max_duration` | MAX_SAMPLES 边界检测 |
| `test_clear` | 清空缓冲区 |
| `test_wav_encoding_header` | WAV 头（RIFF/WAVE/fmt/data）正确性 |
| `test_wav_encoding_roundtrip` | PCM 数据编码后完整可还原 |
| `test_base64_encoding` | base64 编码/解码往返一致 |
| `test_duration_ms_preserved` | 编码后 duration_ms 一致 |
| `test_mp3_encoding_header` | MP3 编码非空输出与格式标识 |
| `test_mp3_smaller_than_wav_for_long_audio` | 长音频 MP3 体积小于 WAV |

### AudioRecorder 测试（10 个）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_new_mock_mode` | Mock 模式下初始化成功 |
| `test_new_with_device_id_mock` | Mock 模式下指定 device_id 初始化成功 |
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

1. **压缩依赖于 MP3 编码器支持的采样率/声道** -- 非常规输入配置可能触发自动回退 WAV
2. **录音时长上限 300 秒** -- `MAX_RECORDING_SECONDS = 300`，超过后自动停止。该限制保护内存使用（约 9.6 MB PCM，WAV/base64 路径体积更大）
3. **MVP 不支持 >5 分钟分流** -- 当前单次录音上限 5 分钟，超过后自动停止并进入处理
4. **无 VAD / 静音检测** -- 不会自动跳过静音段，用户需手动控制录音起止
5. **最近邻重采样** -- 当设备采样率非 16 kHz 时使用最近邻插值（nearest-neighbour），精度对语音识别足够但不适用于高保真场景
6. **RMS 级别上限** -- 最多保留 200 个 RMS 级别，超出后从头部移除
7. **无并发录音** -- 同一 `AudioRecorder` 实例同时只能有一个活跃录音会话
