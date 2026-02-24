use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Information about a recording session that was interrupted (e.g. by a crash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryInfo {
    /// Path to the orphaned audio file.
    pub audio_path: PathBuf,
    /// ISO-8601 timestamp from the sidecar metadata.
    pub timestamp: String,
    /// Estimated recording duration in milliseconds (from metadata, if available).
    pub duration_estimate: Option<u64>,
    /// The processing mode that was active when the recording started.
    pub mode: String,
}

/// Sidecar JSON that sits next to each audio file in the cache directory.
///
/// When a recording starts we write `status: "recording"`. On normal
/// completion the status is updated to `"done"`. If we find files that
/// still say `"recording"` at startup, the previous session terminated
/// abnormally.
#[derive(Debug, Deserialize)]
struct SidecarMeta {
    status: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    duration_estimate: Option<u64>,
    #[serde(default)]
    mode: Option<String>,
}

/// Scan the cache directory for audio files whose sidecar JSON indicates an
/// unfinished recording (`status == "recording"`).
///
/// Returns a list of [`RecoveryInfo`] entries that the UI can present in a
/// recovery dialog so the user can choose to re-process or discard them.
pub fn scan_unfinished_recordings(cache_dir: &Path) -> Vec<RecoveryInfo> {
    let mut results = Vec::new();

    let entries = match std::fs::read_dir(cache_dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::debug!(
                path = %cache_dir.display(),
                error = %err,
                "cannot read cache dir for recovery scan"
            );
            return results;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // We only care about audio files (.wav).
        let is_audio = path
            .extension()
            .map(|ext| ext == "wav")
            .unwrap_or(false);
        if !is_audio {
            continue;
        }

        // Look for the matching sidecar: same stem + ".json".
        let sidecar = path.with_extension("json");
        if !sidecar.exists() {
            continue;
        }

        let meta = match std::fs::read_to_string(&sidecar) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let sidecar_meta: SidecarMeta = match serde_json::from_str(&meta) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if sidecar_meta.status != "recording" {
            continue;
        }

        results.push(RecoveryInfo {
            audio_path: path,
            timestamp: sidecar_meta
                .timestamp
                .unwrap_or_default(),
            duration_estimate: sidecar_meta.duration_estimate,
            mode: sidecar_meta
                .mode
                .unwrap_or_else(|| "dictate".to_string()),
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let results = scan_unfinished_recordings(dir.path());
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_nonexistent_dir() {
        let results = scan_unfinished_recordings(Path::new("/tmp/does_not_exist_12345"));
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_finds_unfinished() {
        let dir = tempfile::tempdir().unwrap();

        // Create a "recording" audio + sidecar.
        let audio = dir.path().join("session1.wav");
        let sidecar = dir.path().join("session1.json");
        fs::write(&audio, b"fake wav data").unwrap();
        fs::write(
            &sidecar,
            r#"{"status":"recording","timestamp":"2025-01-01T00:00:00Z","duration_estimate":5000,"mode":"dictate"}"#,
        )
        .unwrap();

        // Create a completed session (should be ignored).
        let audio2 = dir.path().join("session2.wav");
        let sidecar2 = dir.path().join("session2.json");
        fs::write(&audio2, b"fake wav data").unwrap();
        fs::write(&sidecar2, r#"{"status":"done"}"#).unwrap();

        let results = scan_unfinished_recordings(dir.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].audio_path, audio);
        assert_eq!(results[0].mode, "dictate");
        assert_eq!(results[0].duration_estimate, Some(5000));
    }

    #[test]
    fn test_scan_ignores_audio_without_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("orphan.wav");
        fs::write(&audio, b"fake wav data").unwrap();

        let results = scan_unfinished_recordings(dir.path());
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_ignores_non_wav_files() {
        let dir = tempfile::tempdir().unwrap();

        let txt = dir.path().join("notes.txt");
        let sidecar = dir.path().join("notes.json");
        fs::write(&txt, b"some text").unwrap();
        fs::write(&sidecar, r#"{"status":"recording"}"#).unwrap();

        let results = scan_unfinished_recordings(dir.path());
        assert!(results.is_empty());
    }
}
