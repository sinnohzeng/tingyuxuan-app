use crate::error::AudioError;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Sample rate used for all recordings (16 kHz).
const SAMPLE_RATE: u32 = 16_000;
/// Bits per sample (16-bit PCM).
const BITS_PER_SAMPLE: u16 = 16;
/// Number of audio channels (mono).
const CHANNELS: u16 = 1;

/// WAV file writer that wraps `hound::WavWriter` with a buffered output.
///
/// Produces 16 kHz, 16-bit, mono PCM WAV files suitable for speech-to-text
/// services.
pub struct WavFileWriter {
    writer: WavWriter<BufWriter<File>>,
    sample_count: u64,
}

impl WavFileWriter {
    /// Creates a new WAV file at the given path.
    ///
    /// The file is configured for 16 kHz, 16-bit, mono PCM audio.
    pub fn new(path: &Path) -> Result<Self, AudioError> {
        let spec = WavSpec {
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: BITS_PER_SAMPLE,
            sample_format: SampleFormat::Int,
        };

        let writer = WavWriter::create(path, spec).map_err(|e| {
            AudioError::WavWriteError(format!("Failed to create WAV file at {:?}: {}", path, e))
        })?;

        Ok(Self {
            writer,
            sample_count: 0,
        })
    }

    /// Writes a slice of 16-bit PCM samples to the WAV file.
    pub fn write_samples(&mut self, samples: &[i16]) -> Result<(), AudioError> {
        for &sample in samples {
            self.writer
                .write_sample(sample)
                .map_err(|e| AudioError::WavWriteError(format!("Failed to write sample: {}", e)))?;
        }
        self.sample_count += samples.len() as u64;
        Ok(())
    }

    /// Flushes the internal buffer to disk without finalizing the WAV header.
    pub fn flush(&mut self) -> Result<(), AudioError> {
        self.writer
            .flush()
            .map_err(|e| AudioError::WavWriteError(format!("Failed to flush WAV writer: {}", e)))
    }

    /// Finalizes the WAV header (writes correct data length) and closes the file.
    ///
    /// This must be called when recording is complete. After calling `finalize`,
    /// the writer is consumed and no further writes are possible.
    pub fn finalize(self) -> Result<(), AudioError> {
        self.writer
            .finalize()
            .map_err(|e| AudioError::WavWriteError(format!("Failed to finalize WAV file: {}", e)))
    }

    /// Returns the total number of samples written so far.
    pub fn sample_count(&self) -> u64 {
        self.sample_count
    }

    /// Calculates the duration in milliseconds based on the number of samples
    /// written and the sample rate.
    pub fn duration_ms(&self) -> u64 {
        if SAMPLE_RATE == 0 {
            return 0;
        }
        (self.sample_count * 1000) / SAMPLE_RATE as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_wav_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wav");

        let writer = WavFileWriter::new(&path).unwrap();
        writer.finalize().unwrap();

        assert!(path.exists());
        // A finalized WAV file with no samples should still have a valid header.
        let reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, CHANNELS);
        assert_eq!(spec.sample_rate, SAMPLE_RATE);
        assert_eq!(spec.bits_per_sample, BITS_PER_SAMPLE);
        assert_eq!(spec.sample_format, SampleFormat::Int);
    }

    #[test]
    fn test_write_and_read_samples() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_samples.wav");

        let samples: Vec<i16> = (0..1600).map(|i| (i % 256) as i16).collect();

        {
            let mut writer = WavFileWriter::new(&path).unwrap();
            writer.write_samples(&samples).unwrap();
            assert_eq!(writer.sample_count(), 1600);
            writer.finalize().unwrap();
        }

        let mut reader = hound::WavReader::open(&path).unwrap();
        let read_samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(read_samples.len(), 1600);
        assert_eq!(read_samples, samples);
    }

    #[test]
    fn test_duration_ms() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_duration.wav");

        let mut writer = WavFileWriter::new(&path).unwrap();

        // Write exactly 1 second of samples at 16kHz.
        let samples = vec![0i16; 16_000];
        writer.write_samples(&samples).unwrap();
        assert_eq!(writer.duration_ms(), 1000);

        // Write another half second.
        let more_samples = vec![0i16; 8_000];
        writer.write_samples(&more_samples).unwrap();
        assert_eq!(writer.duration_ms(), 1500);

        writer.finalize().unwrap();
    }

    #[test]
    fn test_flush() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_flush.wav");

        let mut writer = WavFileWriter::new(&path).unwrap();
        let samples = vec![0i16; 160];
        writer.write_samples(&samples).unwrap();
        writer.flush().unwrap();

        // File should exist and have some data even before finalize.
        assert!(path.exists());
        assert!(std::fs::metadata(&path).unwrap().len() > 0);

        writer.finalize().unwrap();
    }

    #[test]
    fn test_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_empty.wav");

        let writer = WavFileWriter::new(&path).unwrap();
        assert_eq!(writer.sample_count(), 0);
        assert_eq!(writer.duration_ms(), 0);
        writer.finalize().unwrap();
    }

    #[test]
    fn test_create_fails_on_invalid_path() {
        let result = WavFileWriter::new(Path::new("/nonexistent/dir/test.wav"));
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_write_calls() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_multi.wav");

        let mut writer = WavFileWriter::new(&path).unwrap();

        // Write in multiple small batches.
        for _ in 0..10 {
            let chunk = vec![100i16; 160];
            writer.write_samples(&chunk).unwrap();
        }

        assert_eq!(writer.sample_count(), 1600);
        // 1600 samples at 16kHz = 100ms.
        assert_eq!(writer.duration_ms(), 100);

        writer.finalize().unwrap();

        // Verify all samples were written correctly.
        let mut reader = hound::WavReader::open(&path).unwrap();
        let read_samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(read_samples.len(), 1600);
        assert!(read_samples.iter().all(|&s| s == 100));
    }
}
