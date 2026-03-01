use base64::Engine;

use crate::error::AudioError;

/// 音频编码格式枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// PCM + RIFF/WAVE 头（44 字节头 + raw PCM16）。
    Wav,
}

/// 录音缓冲区：在录音过程中累积 PCM 采样。
pub struct AudioBuffer {
    samples: Vec<i16>,
    sample_rate: u32,
    channels: u16,
}

/// 录音时长上限（秒）。超过后自动停止录音。
pub const MAX_RECORDING_SECONDS: u64 = 300;
/// 录音采样上限（16kHz × 300s = 4_800_000 samples）。
pub const MAX_SAMPLES: usize = 16_000 * MAX_RECORDING_SECONDS as usize;

impl AudioBuffer {
    /// 创建新的音频缓冲区。
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
            channels,
        }
    }

    /// 追加 PCM 采样。
    pub fn push_samples(&mut self, samples: &[i16]) {
        self.samples.extend_from_slice(samples);
    }

    /// 当前缓冲区的采样数。
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// 缓冲区是否为空。
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// 当前录音时长（毫秒）。
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        (self.samples.len() as u64 * 1000) / self.sample_rate as u64
    }

    /// 是否超过录音时长上限。
    pub fn exceeds_max_duration(&self) -> bool {
        self.samples.len() >= MAX_SAMPLES
    }

    /// 编码为指定格式的字节序列。
    pub fn encode(&self, format: AudioFormat) -> Result<EncodedAudio, AudioError> {
        match format {
            AudioFormat::Wav => self.encode_wav(),
        }
    }

    /// 清空缓冲区，释放内存。
    pub fn clear(&mut self) {
        self.samples.clear();
        self.samples.shrink_to_fit();
    }

    /// WAV 编码：44 字节 RIFF/WAVE 头 + raw PCM16 data。
    fn encode_wav(&self) -> Result<EncodedAudio, AudioError> {
        let bits_per_sample: u16 = 16;
        let byte_rate = self.sample_rate * self.channels as u32 * 2;
        let block_align = self.channels * 2;
        let data_size = u32::try_from(self.samples.len() * 2).map_err(|_| {
            AudioError::StreamError(format!(
                "音频数据过大：{} 字节超出 WAV 格式 4GB 限制",
                self.samples.len() * 2
            ))
        })?;
        let file_size = 36 + data_size;

        let mut buf = Vec::with_capacity(44 + data_size as usize);

        // RIFF header
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&file_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");

        // fmt sub-chunk
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes()); // sub-chunk size
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        buf.extend_from_slice(&self.channels.to_le_bytes());
        buf.extend_from_slice(&self.sample_rate.to_le_bytes());
        buf.extend_from_slice(&byte_rate.to_le_bytes());
        buf.extend_from_slice(&block_align.to_le_bytes());
        buf.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data sub-chunk
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        for &sample in &self.samples {
            buf.extend_from_slice(&sample.to_le_bytes());
        }

        Ok(EncodedAudio {
            data: buf,
            format: AudioFormat::Wav,
            duration_ms: self.duration_ms(),
        })
    }
}

/// 编码后的音频数据。
#[derive(Debug)]
pub struct EncodedAudio {
    pub data: Vec<u8>,
    pub format: AudioFormat,
    pub duration_ms: u64,
}

impl EncodedAudio {
    /// base64 编码，用于 API 请求体。
    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.data)
    }

    /// 返回 OpenAI API 期望的格式字符串。
    pub fn format_str(&self) -> &'static str {
        match self.format {
            AudioFormat::Wav => "wav",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        let buf = AudioBuffer::new(16_000, 1);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.duration_ms(), 0);
    }

    #[test]
    fn test_push_samples() {
        let mut buf = AudioBuffer::new(16_000, 1);
        buf.push_samples(&[100, 200, 300]);
        assert_eq!(buf.len(), 3);
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_duration_ms() {
        let mut buf = AudioBuffer::new(16_000, 1);
        // 16000 samples = 1 second = 1000ms
        buf.push_samples(&vec![0i16; 16_000]);
        assert_eq!(buf.duration_ms(), 1000);
    }

    #[test]
    fn test_exceeds_max_duration() {
        let mut buf = AudioBuffer::new(16_000, 1);
        assert!(!buf.exceeds_max_duration());
        buf.push_samples(&vec![0i16; MAX_SAMPLES]);
        assert!(buf.exceeds_max_duration());
    }

    #[test]
    fn test_clear() {
        let mut buf = AudioBuffer::new(16_000, 1);
        buf.push_samples(&vec![0i16; 1000]);
        assert_eq!(buf.len(), 1000);
        buf.clear();
        assert!(buf.is_empty());
    }

    #[test]
    fn test_wav_encoding_header() {
        let mut buf = AudioBuffer::new(16_000, 1);
        buf.push_samples(&[0i16; 320]); // 20ms

        let encoded = buf.encode(AudioFormat::Wav).unwrap();
        assert_eq!(encoded.format, AudioFormat::Wav);
        assert_eq!(encoded.format_str(), "wav");

        // WAV header validation
        assert_eq!(&encoded.data[0..4], b"RIFF");
        assert_eq!(&encoded.data[8..12], b"WAVE");
        assert_eq!(&encoded.data[12..16], b"fmt ");
        assert_eq!(&encoded.data[36..40], b"data");

        // Total size: 44 header + 320 samples * 2 bytes = 684
        assert_eq!(encoded.data.len(), 44 + 320 * 2);
    }

    #[test]
    fn test_wav_encoding_roundtrip() {
        let mut buf = AudioBuffer::new(16_000, 1);
        let samples: Vec<i16> = (0..100).map(|i| (i * 100) as i16).collect();
        buf.push_samples(&samples);

        let encoded = buf.encode(AudioFormat::Wav).unwrap();

        // 验证 PCM 数据完整性（跳过 44 字节头）
        for (i, &sample) in samples.iter().enumerate() {
            let offset = 44 + i * 2;
            let stored = i16::from_le_bytes([encoded.data[offset], encoded.data[offset + 1]]);
            assert_eq!(stored, sample);
        }
    }

    #[test]
    fn test_base64_encoding() {
        let mut buf = AudioBuffer::new(16_000, 1);
        buf.push_samples(&[0i16; 10]);

        let encoded = buf.encode(AudioFormat::Wav).unwrap();
        let b64 = encoded.to_base64();
        assert!(!b64.is_empty());

        // 验证 base64 可解码
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .unwrap();
        assert_eq!(decoded, encoded.data);
    }

    #[test]
    fn test_duration_ms_preserved() {
        let mut buf = AudioBuffer::new(16_000, 1);
        buf.push_samples(&vec![0i16; 48_000]); // 3 seconds
        assert_eq!(buf.duration_ms(), 3000);

        let encoded = buf.encode(AudioFormat::Wav).unwrap();
        assert_eq!(encoded.duration_ms, 3000);
    }
}
