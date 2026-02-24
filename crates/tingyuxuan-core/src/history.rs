use crate::config::AppConfig;
use crate::error::HistoryError;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// A single transcript record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    pub id: String,
    pub timestamp: String,
    pub mode: String,
    pub raw_text: Option<String>,
    pub processed_text: Option<String>,
    pub audio_path: Option<String>,
    pub status: String,
    pub app_context: Option<String>,
    pub duration_ms: Option<i64>,
    pub language: Option<String>,
    pub error_message: Option<String>,
}

pub struct HistoryManager {
    conn: Connection,
}

impl HistoryManager {
    /// Create a new HistoryManager with a file-based SQLite database.
    pub fn new() -> Result<Self, HistoryError> {
        let data_dir = AppConfig::data_dir().map_err(|e| {
            HistoryError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))
        })?;
        let db_dir = data_dir.join("history");
        std::fs::create_dir_all(&db_dir)?;
        let db_path = db_dir.join("transcripts.db");
        let conn = Connection::open(db_path)?;
        let manager = Self { conn };
        manager.init_tables()?;
        Ok(manager)
    }

    /// Create a HistoryManager with an in-memory database (for testing).
    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self, HistoryError> {
        let conn = Connection::open_in_memory()?;
        let manager = Self { conn };
        manager.init_tables()?;
        Ok(manager)
    }

    fn init_tables(&self) -> Result<(), HistoryError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS transcripts (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                mode TEXT NOT NULL,
                raw_text TEXT,
                processed_text TEXT,
                audio_path TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                app_context TEXT,
                duration_ms INTEGER,
                language TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_transcripts_status ON transcripts(status);
            CREATE INDEX IF NOT EXISTS idx_transcripts_timestamp ON transcripts(timestamp DESC);",
        )?;
        Ok(())
    }

    /// Save a new transcript record.
    pub fn save_transcript(&self, record: &TranscriptRecord) -> Result<(), HistoryError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO transcripts
             (id, timestamp, mode, raw_text, processed_text, audio_path,
              status, app_context, duration_ms, language, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.id,
                record.timestamp,
                record.mode,
                record.raw_text,
                record.processed_text,
                record.audio_path,
                record.status,
                record.app_context,
                record.duration_ms,
                record.language,
                record.error_message,
            ],
        )?;
        Ok(())
    }

    /// Get the most recent transcript records.
    pub fn get_recent(&self, limit: u32) -> Result<Vec<TranscriptRecord>, HistoryError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, mode, raw_text, processed_text, audio_path,
                    status, app_context, duration_ms, language, error_message
             FROM transcripts
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let records = stmt
            .query_map(params![limit], |row| {
                Ok(TranscriptRecord {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    mode: row.get(2)?,
                    raw_text: row.get(3)?,
                    processed_text: row.get(4)?,
                    audio_path: row.get(5)?,
                    status: row.get(6)?,
                    app_context: row.get(7)?,
                    duration_ms: row.get(8)?,
                    language: row.get(9)?,
                    error_message: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Get all pending/failed transcript records.
    pub fn get_pending(&self) -> Result<Vec<TranscriptRecord>, HistoryError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, mode, raw_text, processed_text, audio_path,
                    status, app_context, duration_ms, language, error_message
             FROM transcripts
             WHERE status IN ('pending', 'failed')
             ORDER BY timestamp DESC",
        )?;

        let records = stmt
            .query_map([], |row| {
                Ok(TranscriptRecord {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    mode: row.get(2)?,
                    raw_text: row.get(3)?,
                    processed_text: row.get(4)?,
                    audio_path: row.get(5)?,
                    status: row.get(6)?,
                    app_context: row.get(7)?,
                    duration_ms: row.get(8)?,
                    language: row.get(9)?,
                    error_message: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Update the status of a transcript record.
    pub fn update_status(&self, id: &str, status: &str) -> Result<(), HistoryError> {
        self.conn.execute(
            "UPDATE transcripts SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    /// Update a transcript record with processed text.
    pub fn update_processed(
        &self,
        id: &str,
        raw_text: &str,
        processed_text: &str,
    ) -> Result<(), HistoryError> {
        self.conn.execute(
            "UPDATE transcripts SET raw_text = ?1, processed_text = ?2, status = 'success'
             WHERE id = ?3",
            params![raw_text, processed_text, id],
        )?;
        Ok(())
    }

    /// Update a transcript record with only the processed text and mark as success.
    ///
    /// This is a convenience method when the raw text is not available at the
    /// call site (e.g. when the pipeline only returns the final processed text).
    pub fn update_result(&self, id: &str, processed_text: &str) -> Result<(), HistoryError> {
        self.conn.execute(
            "UPDATE transcripts SET processed_text = ?1, status = 'success' WHERE id = ?2",
            params![processed_text, id],
        )?;
        Ok(())
    }

    /// Delete a transcript record by ID.
    pub fn delete(&self, id: &str) -> Result<(), HistoryError> {
        self.conn
            .execute("DELETE FROM transcripts WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Get total count of records.
    pub fn count(&self) -> Result<u64, HistoryError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM transcripts", [], |row| row.get(0))?;
        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(id: &str, status: &str) -> TranscriptRecord {
        TranscriptRecord {
            id: id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            mode: "dictate".to_string(),
            raw_text: Some("hello world".to_string()),
            processed_text: Some("Hello, world.".to_string()),
            audio_path: Some("/tmp/test.wav".to_string()),
            status: status.to_string(),
            app_context: None,
            duration_ms: Some(3000),
            language: Some("en".to_string()),
            error_message: None,
        }
    }

    #[test]
    fn test_create_and_query() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        let record = make_record("test-1", "success");
        mgr.save_transcript(&record).unwrap();

        let recent = mgr.get_recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, "test-1");
        assert_eq!(recent[0].raw_text, Some("hello world".to_string()));
    }

    #[test]
    fn test_get_pending() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        mgr.save_transcript(&make_record("r1", "success")).unwrap();
        mgr.save_transcript(&make_record("r2", "pending")).unwrap();
        mgr.save_transcript(&make_record("r3", "failed")).unwrap();

        let pending = mgr.get_pending().unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_update_status() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        mgr.save_transcript(&make_record("r1", "pending")).unwrap();
        mgr.update_status("r1", "success").unwrap();

        let recent = mgr.get_recent(10).unwrap();
        assert_eq!(recent[0].status, "success");
    }

    #[test]
    fn test_delete() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        mgr.save_transcript(&make_record("r1", "success")).unwrap();
        assert_eq!(mgr.count().unwrap(), 1);
        mgr.delete("r1").unwrap();
        assert_eq!(mgr.count().unwrap(), 0);
    }
}
