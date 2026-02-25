use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use crate::config::AppConfig;
use crate::llm::provider::ProcessingMode;

/// A recording that was captured while the device was offline and is waiting
/// to be processed once connectivity is restored.
#[derive(Debug, Clone)]
pub struct QueuedRecording {
    pub session_id: String,
    pub audio_path: PathBuf,
    pub mode: ProcessingMode,
    pub target_language: Option<String>,
    pub selected_text: Option<String>,
    pub app_context: Option<String>,
}

/// SQL schema for the queue table.
const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS queue (
    session_id     TEXT PRIMARY KEY,
    audio_path     TEXT NOT NULL,
    mode           TEXT NOT NULL,
    target_language TEXT,
    selected_text  TEXT,
    app_context    TEXT,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);";

/// A persistent FIFO queue of recordings captured while offline, backed by SQLite.
///
/// Queued recordings survive application crashes and restarts.  When the
/// network comes back, the orchestrator calls [`drain`](Self::drain) to
/// retrieve all pending items and re-submit them through the pipeline.
///
/// Falls back to an in-memory SQLite database if the file-based database
/// cannot be created (e.g. read-only filesystem).
pub struct OfflineQueue {
    conn: Connection,
}

impl OfflineQueue {
    /// Create a new persistent queue using the application data directory.
    ///
    /// If the file-based database cannot be opened, automatically falls back
    /// to an in-memory database and logs a warning.
    pub fn new() -> Self {
        match Self::open_persistent() {
            Ok(q) => q,
            Err(e) => {
                tracing::warn!(
                    "Failed to open persistent queue database: {} — falling back to in-memory",
                    e
                );
                Self::new_in_memory()
            }
        }
    }

    /// Open a file-based queue database in the application data directory.
    fn open_persistent() -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = AppConfig::data_dir()?;
        let db_dir = data_dir.join("queue");
        std::fs::create_dir_all(&db_dir)?;
        let db_path = db_dir.join("offline_queue.db");
        Self::open_file(&db_path).map_err(|e| e.into())
    }

    /// Open a queue database at a specific file path.
    fn open_file(db_path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Create an in-memory queue (for testing or as a fallback).
    pub fn new_in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("failed to open in-memory SQLite");
        conn.execute_batch(SCHEMA)
            .expect("failed to create queue schema");
        Self { conn }
    }

    /// Append a recording to the end of the queue.
    ///
    /// If a recording with the same `session_id` already exists, it is replaced.
    pub fn enqueue(&mut self, recording: QueuedRecording) {
        if let Err(e) = self.conn.execute(
            "INSERT OR REPLACE INTO queue \
             (session_id, audio_path, mode, target_language, selected_text, app_context) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                recording.session_id,
                recording.audio_path.to_string_lossy().as_ref(),
                mode_to_str(&recording.mode),
                recording.target_language,
                recording.selected_text,
                recording.app_context,
            ],
        ) {
            tracing::error!("Failed to enqueue recording: {}", e);
        }
    }

    /// Remove **all** queued recordings and return them in FIFO order.
    ///
    /// The SELECT and DELETE happen inside a single transaction so no items
    /// are lost if the application crashes mid-drain.
    pub fn drain(&mut self) -> Vec<QueuedRecording> {
        let tx = match self.conn.transaction() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!("Failed to begin drain transaction: {}", e);
                return Vec::new();
            }
        };

        let items: Vec<QueuedRecording>;
        {
            let mut stmt = match tx.prepare(
                "SELECT session_id, audio_path, mode, target_language, selected_text, app_context \
                 FROM queue ORDER BY created_at ASC",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to prepare drain query: {}", e);
                    return Vec::new();
                }
            };

            items = match stmt.query_map([], |row| {
                Ok(QueuedRecording {
                    session_id: row.get(0)?,
                    audio_path: PathBuf::from(row.get::<_, String>(1)?),
                    mode: str_to_mode(&row.get::<_, String>(2)?),
                    target_language: row.get(3)?,
                    selected_text: row.get(4)?,
                    app_context: row.get(5)?,
                })
            }) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(e) => {
                    tracing::error!("Failed to query drain: {}", e);
                    return Vec::new();
                }
            };
        }

        if let Err(e) = tx.execute("DELETE FROM queue", []) {
            tracing::error!("Failed to delete drained items: {}", e);
            return Vec::new();
        }
        if let Err(e) = tx.commit() {
            tracing::error!("Failed to commit drain transaction: {}", e);
            return Vec::new();
        }

        items
    }

    /// Number of recordings currently waiting in the queue.
    pub fn len(&self) -> usize {
        self.conn
            .query_row("SELECT COUNT(*) FROM queue", [], |row| {
                row.get::<_, usize>(0)
            })
            .unwrap_or(0)
    }

    /// Returns `true` when there are no queued recordings.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for OfflineQueue {
    fn default() -> Self {
        Self::new()
    }
}

fn mode_to_str(mode: &ProcessingMode) -> &'static str {
    match mode {
        ProcessingMode::Dictate => "dictate",
        ProcessingMode::Translate => "translate",
        ProcessingMode::AiAssistant => "ai_assistant",
        ProcessingMode::Edit => "edit",
    }
}

fn str_to_mode(s: &str) -> ProcessingMode {
    match s {
        "translate" => ProcessingMode::Translate,
        "ai_assistant" => ProcessingMode::AiAssistant,
        "edit" => ProcessingMode::Edit,
        _ => ProcessingMode::Dictate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recording(id: &str) -> QueuedRecording {
        QueuedRecording {
            session_id: id.to_string(),
            audio_path: PathBuf::from(format!("/tmp/{id}.wav")),
            mode: ProcessingMode::Dictate,
            target_language: None,
            selected_text: None,
            app_context: None,
        }
    }

    #[test]
    fn new_queue_is_empty() {
        let q = OfflineQueue::new_in_memory();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn enqueue_increases_length() {
        let mut q = OfflineQueue::new_in_memory();
        q.enqueue(sample_recording("a"));
        assert_eq!(q.len(), 1);
        assert!(!q.is_empty());

        q.enqueue(sample_recording("b"));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn drain_returns_all_items_in_order() {
        let mut q = OfflineQueue::new_in_memory();
        q.enqueue(sample_recording("1"));
        q.enqueue(sample_recording("2"));
        q.enqueue(sample_recording("3"));

        let items = q.drain();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].session_id, "1");
        assert_eq!(items[1].session_id, "2");
        assert_eq!(items[2].session_id, "3");

        // Queue should be empty after drain.
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn drain_on_empty_queue_returns_empty_vec() {
        let mut q = OfflineQueue::new_in_memory();
        let items = q.drain();
        assert!(items.is_empty());
    }

    #[test]
    fn enqueue_after_drain_works() {
        let mut q = OfflineQueue::new_in_memory();
        q.enqueue(sample_recording("x"));
        let _ = q.drain();

        q.enqueue(sample_recording("y"));
        assert_eq!(q.len(), 1);
        let items = q.drain();
        assert_eq!(items[0].session_id, "y");
    }

    #[test]
    fn duplicate_session_id_replaces() {
        let mut q = OfflineQueue::new_in_memory();
        q.enqueue(QueuedRecording {
            session_id: "dup".to_string(),
            audio_path: PathBuf::from("/tmp/first.wav"),
            mode: ProcessingMode::Dictate,
            target_language: None,
            selected_text: None,
            app_context: None,
        });
        q.enqueue(QueuedRecording {
            session_id: "dup".to_string(),
            audio_path: PathBuf::from("/tmp/second.wav"),
            mode: ProcessingMode::Translate,
            target_language: Some("en".to_string()),
            selected_text: None,
            app_context: None,
        });

        assert_eq!(q.len(), 1);
        let items = q.drain();
        assert_eq!(items[0].audio_path, PathBuf::from("/tmp/second.wav"));
        assert!(matches!(items[0].mode, ProcessingMode::Translate));
    }

    #[test]
    fn mode_roundtrip() {
        let mut q = OfflineQueue::new_in_memory();
        q.enqueue(QueuedRecording {
            session_id: "t1".to_string(),
            audio_path: PathBuf::from("/tmp/t1.wav"),
            mode: ProcessingMode::Translate,
            target_language: Some("ja".to_string()),
            selected_text: None,
            app_context: Some("Firefox".to_string()),
        });
        q.enqueue(QueuedRecording {
            session_id: "t2".to_string(),
            audio_path: PathBuf::from("/tmp/t2.wav"),
            mode: ProcessingMode::AiAssistant,
            target_language: None,
            selected_text: Some("hello".to_string()),
            app_context: None,
        });
        q.enqueue(QueuedRecording {
            session_id: "t3".to_string(),
            audio_path: PathBuf::from("/tmp/t3.wav"),
            mode: ProcessingMode::Edit,
            target_language: None,
            selected_text: Some("old text".to_string()),
            app_context: None,
        });

        let items = q.drain();
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0].mode, ProcessingMode::Translate));
        assert_eq!(items[0].target_language, Some("ja".to_string()));
        assert_eq!(items[0].app_context, Some("Firefox".to_string()));
        assert!(matches!(items[1].mode, ProcessingMode::AiAssistant));
        assert_eq!(items[1].selected_text, Some("hello".to_string()));
        assert!(matches!(items[2].mode, ProcessingMode::Edit));
        assert_eq!(items[2].selected_text, Some("old text".to_string()));
    }

    #[test]
    fn persistence_across_operations() {
        let mut q = OfflineQueue::new_in_memory();

        // Enqueue, drain partially, enqueue more, drain again
        q.enqueue(sample_recording("a"));
        q.enqueue(sample_recording("b"));
        assert_eq!(q.len(), 2);

        let batch1 = q.drain();
        assert_eq!(batch1.len(), 2);
        assert!(q.is_empty());

        q.enqueue(sample_recording("c"));
        q.enqueue(sample_recording("d"));
        q.enqueue(sample_recording("e"));
        assert_eq!(q.len(), 3);

        let batch2 = q.drain();
        assert_eq!(batch2.len(), 3);
        assert_eq!(batch2[0].session_id, "c");
        assert_eq!(batch2[1].session_id, "d");
        assert_eq!(batch2[2].session_id, "e");
    }
}
