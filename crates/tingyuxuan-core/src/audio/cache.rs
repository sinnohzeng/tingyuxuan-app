use crate::config::AppConfig;
use crate::error::AudioError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// Manages the lifecycle of audio recording cache files.
///
/// Audio files are stored in `{data_dir}/cache/audio/` alongside sidecar JSON
/// metadata files (`*.wav.json`).
pub struct AudioCache {
    cache_dir: PathBuf,
}

impl AudioCache {
    /// Creates a new `AudioCache`, ensuring the cache directory exists.
    ///
    /// The cache directory is located at `{data_dir}/cache/audio/` where
    /// `data_dir` comes from [`AppConfig::data_dir`].
    pub fn new() -> Result<Self, AudioError> {
        let data_dir = AppConfig::data_dir().map_err(|e| {
            AudioError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Could not determine data directory: {}", e),
            ))
        })?;
        let cache_dir = data_dir.join("cache").join("audio");
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Creates an `AudioCache` at a caller-specified directory.
    ///
    /// This is primarily useful for testing.
    pub fn with_dir(cache_dir: PathBuf) -> Result<Self, AudioError> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Returns the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Generates the path for a new audio recording file.
    ///
    /// Format: `{cache_dir}/{ISO_timestamp}_{mode}_{session_id}.wav`
    pub fn audio_path(&self, mode: &str, session_id: &str) -> PathBuf {
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let filename = format!("{}_{}_{}_.wav", timestamp, mode, session_id);
        self.cache_dir.join(filename)
    }

    /// Writes a sidecar JSON metadata file for an audio recording.
    ///
    /// The metadata file is placed next to the WAV file with a `.json`
    /// extension appended (e.g. `recording.wav.json`).
    ///
    /// On Unix platforms the file permissions are set to `0600`.
    pub fn write_metadata(
        &self,
        audio_path: &Path,
        mode: &str,
        status: &str,
        duration_ms: u64,
    ) -> Result<(), AudioError> {
        let now = chrono::Utc::now().to_rfc3339();
        let meta = AudioMetadata {
            mode: mode.to_string(),
            status: status.to_string(),
            duration_ms,
            created_at: now.clone(),
            updated_at: now,
        };

        let meta_path = sidecar_path(audio_path);
        let json = serde_json::to_string_pretty(&meta).map_err(|e| {
            AudioError::WavWriteError(format!("Failed to serialize metadata: {}", e))
        })?;

        std::fs::write(&meta_path, &json)?;
        set_restrictive_permissions(&meta_path)?;

        Ok(())
    }

    /// Updates the `status` field in an existing sidecar metadata file.
    pub fn update_status(&self, audio_path: &Path, status: &str) -> Result<(), AudioError> {
        let meta_path = sidecar_path(audio_path);
        let contents = std::fs::read_to_string(&meta_path)?;
        let mut meta: AudioMetadata = serde_json::from_str(&contents)
            .map_err(|e| AudioError::WavWriteError(format!("Failed to parse metadata: {}", e)))?;

        meta.status = status.to_string();
        meta.updated_at = chrono::Utc::now().to_rfc3339();

        let json = serde_json::to_string_pretty(&meta).map_err(|e| {
            AudioError::WavWriteError(format!("Failed to serialize metadata: {}", e))
        })?;
        std::fs::write(&meta_path, &json)?;

        Ok(())
    }

    /// Scans the cache directory and returns paths of audio files whose sidecar
    /// status is `"recording"` or `"failed"`.
    pub fn list_pending(&self) -> Result<Vec<PathBuf>, AudioError> {
        let mut pending = Vec::new();

        let entries = std::fs::read_dir(&self.cache_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only look at .wav files.
            if path.extension().and_then(|e| e.to_str()) != Some("wav") {
                continue;
            }

            let meta_path = sidecar_path(&path);
            if !meta_path.exists() {
                continue;
            }

            if let Ok(contents) = std::fs::read_to_string(&meta_path)
                && let Ok(meta) = serde_json::from_str::<AudioMetadata>(&contents)
                && (meta.status == "recording" || meta.status == "failed")
            {
                pending.push(path);
            }
        }

        Ok(pending)
    }

    /// Deletes audio files (and their sidecars) that are older than
    /// `max_age_hours`.
    ///
    /// Age is determined by the filesystem modification time.
    pub fn cleanup_expired(&self, max_age_hours: u64) -> Result<u64, AudioError> {
        let max_age = std::time::Duration::from_secs(max_age_hours * 3600);
        let now = std::time::SystemTime::now();
        let mut removed: u64 = 0;

        let entries = std::fs::read_dir(&self.cache_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only process .wav files; the sidecar will be removed alongside.
            if path.extension().and_then(|e| e.to_str()) != Some("wav") {
                continue;
            }

            let metadata = std::fs::metadata(&path)?;
            let modified = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

            if let Ok(age) = now.duration_since(modified)
                && age > max_age
            {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_file(sidecar_path(&path));
                removed += 1;
            }
        }

        Ok(removed)
    }
}

/// Returns the sidecar JSON path for a given audio file path.
///
/// Example: `/cache/audio/recording.wav` -> `/cache/audio/recording.wav.json`
fn sidecar_path(audio_path: &Path) -> PathBuf {
    let mut p = audio_path.as_os_str().to_owned();
    p.push(".json");
    PathBuf::from(p)
}

/// Sets restrictive file permissions (0600) on Unix platforms.
#[cfg(unix)]
fn set_restrictive_permissions(path: &Path) -> Result<(), AudioError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
fn set_restrictive_permissions(_path: &Path) -> Result<(), AudioError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_cache(dir: &Path) -> AudioCache {
        AudioCache::with_dir(dir.to_path_buf()).expect("should create test cache")
    }

    #[test]
    fn test_with_dir_creates_directory() {
        let dir = tempdir().unwrap();
        let cache_dir = dir.path().join("sub").join("audio");
        let cache = AudioCache::with_dir(cache_dir.clone()).unwrap();
        assert!(cache_dir.exists());
        assert_eq!(cache.cache_dir(), cache_dir);
    }

    #[test]
    fn test_audio_path_format() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        let path = cache.audio_path("dictate", "abc123");
        let filename = path.file_name().unwrap().to_string_lossy();

        assert!(filename.contains("dictate"));
        assert!(filename.contains("abc123"));
        assert!(filename.ends_with(".wav"));
        assert!(path.starts_with(dir.path()));
    }

    #[test]
    fn test_write_and_read_metadata() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        let audio_path = dir.path().join("test.wav");
        // Create a dummy WAV file so the path exists.
        std::fs::write(&audio_path, b"dummy").unwrap();

        cache
            .write_metadata(&audio_path, "dictate", "recording", 0)
            .unwrap();

        let meta_path = sidecar_path(&audio_path);
        assert!(meta_path.exists());

        let contents = std::fs::read_to_string(&meta_path).unwrap();
        let meta: AudioMetadata = serde_json::from_str(&contents).unwrap();
        assert_eq!(meta.mode, "dictate");
        assert_eq!(meta.status, "recording");
        assert_eq!(meta.duration_ms, 0);
    }

    #[test]
    fn test_update_status() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        let audio_path = dir.path().join("test2.wav");
        std::fs::write(&audio_path, b"dummy").unwrap();

        cache
            .write_metadata(&audio_path, "translate", "recording", 0)
            .unwrap();

        cache.update_status(&audio_path, "completed").unwrap();

        let contents = std::fs::read_to_string(sidecar_path(&audio_path)).unwrap();
        let meta: AudioMetadata = serde_json::from_str(&contents).unwrap();
        assert_eq!(meta.status, "completed");
        // mode should not have changed.
        assert_eq!(meta.mode, "translate");
    }

    #[test]
    fn test_list_pending() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        // Create three files with different statuses.
        for (name, status) in &[
            ("a.wav", "recording"),
            ("b.wav", "completed"),
            ("c.wav", "failed"),
        ] {
            let p = dir.path().join(name);
            std::fs::write(&p, b"dummy").unwrap();
            cache.write_metadata(&p, "dictate", status, 100).unwrap();
        }

        let pending = cache.list_pending().unwrap();
        assert_eq!(pending.len(), 2);

        let names: Vec<String> = pending
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"a.wav".to_string()));
        assert!(names.contains(&"c.wav".to_string()));
    }

    #[test]
    fn test_list_pending_no_sidecar() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        // A .wav without a sidecar should be ignored.
        let p = dir.path().join("orphan.wav");
        std::fs::write(&p, b"dummy").unwrap();

        let pending = cache.list_pending().unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn test_cleanup_expired() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        // Create a file and its sidecar.
        let p = dir.path().join("old.wav");
        std::fs::write(&p, b"dummy").unwrap();
        cache
            .write_metadata(&p, "dictate", "completed", 1000)
            .unwrap();

        // With max_age_hours=0, everything is expired.
        // We need to set the modification time to the past, but since we just
        // created it, max_age=0 means "older than 0 hours" which is everything.
        // However, duration_since might be very small. Use a small sleep or
        // just set max_age to 0 and accept that the file was created "now".
        //
        // Actually, max_age_hours=0 means max_age = Duration::ZERO, so any
        // file with age > 0 (even 1 nanosecond) will be removed.
        std::thread::sleep(std::time::Duration::from_millis(10));
        let removed = cache.cleanup_expired(0).unwrap();
        assert_eq!(removed, 1);
        assert!(!p.exists());
        assert!(!sidecar_path(&p).exists());
    }

    #[test]
    fn test_cleanup_keeps_fresh_files() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        let p = dir.path().join("fresh.wav");
        std::fs::write(&p, b"dummy").unwrap();
        cache
            .write_metadata(&p, "dictate", "completed", 500)
            .unwrap();

        // 24 hours is far longer than the file has existed.
        let removed = cache.cleanup_expired(24).unwrap();
        assert_eq!(removed, 0);
        assert!(p.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_metadata_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        let p = dir.path().join("perms.wav");
        std::fs::write(&p, b"dummy").unwrap();
        cache.write_metadata(&p, "dictate", "recording", 0).unwrap();

        let meta_path = sidecar_path(&p);
        let perms = std::fs::metadata(&meta_path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_sidecar_path() {
        let p = PathBuf::from("/tmp/audio/test.wav");
        assert_eq!(sidecar_path(&p), PathBuf::from("/tmp/audio/test.wav.json"));
    }

    #[test]
    fn test_update_status_nonexistent_sidecar() {
        let dir = tempdir().unwrap();
        let cache = test_cache(dir.path());

        let p = dir.path().join("noside.wav");
        let result = cache.update_status(&p, "completed");
        assert!(result.is_err());
    }
}
