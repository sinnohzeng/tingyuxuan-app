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

    /// Get a single transcript record by ID.
    pub fn get_by_id(&self, id: &str) -> Result<Option<TranscriptRecord>, HistoryError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, mode, raw_text, processed_text, audio_path,
                    status, app_context, duration_ms, language, error_message
             FROM transcripts WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(params![id], |row| {
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
        })?;

        match rows.next() {
            Some(Ok(record)) => Ok(Some(record)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
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

    /// Full-text search across raw_text and processed_text.
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<TranscriptRecord>, HistoryError> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, mode, raw_text, processed_text, audio_path,
                    status, app_context, duration_ms, language, error_message
             FROM transcripts
             WHERE raw_text LIKE ?1 OR processed_text LIKE ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )?;

        let records = stmt
            .query_map(params![pattern, limit], |row| {
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

    /// Paginated query with LIMIT and OFFSET.
    pub fn get_page(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<TranscriptRecord>, HistoryError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, mode, raw_text, processed_text, audio_path,
                    status, app_context, duration_ms, language, error_message
             FROM transcripts
             ORDER BY timestamp DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let records = stmt
            .query_map(params![limit, offset], |row| {
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

    /// Delete multiple records by IDs. Returns the number of deleted rows.
    pub fn delete_batch(&self, ids: &[String]) -> Result<u64, HistoryError> {
        if ids.is_empty() {
            return Ok(0);
        }
        // Build parameterized placeholders: (?1, ?2, ?3, ...)
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "DELETE FROM transcripts WHERE id IN ({})",
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let deleted = self.conn.execute(&sql, params.as_slice())?;
        Ok(deleted as u64)
    }

    /// Delete all records. Returns the number of deleted rows.
    pub fn clear_all(&self) -> Result<u64, HistoryError> {
        let deleted = self.conn.execute("DELETE FROM transcripts", [])?;
        Ok(deleted as u64)
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
    fn test_get_by_id_found() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        mgr.save_transcript(&make_record("r1", "success")).unwrap();

        let record = mgr.get_by_id("r1").unwrap();
        assert!(record.is_some());
        let record = record.unwrap();
        assert_eq!(record.id, "r1");
        assert_eq!(record.status, "success");
    }

    #[test]
    fn test_get_by_id_not_found() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        let record = mgr.get_by_id("nonexistent").unwrap();
        assert!(record.is_none());
    }

    #[test]
    fn test_delete() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        mgr.save_transcript(&make_record("r1", "success")).unwrap();
        assert_eq!(mgr.count().unwrap(), 1);
        mgr.delete("r1").unwrap();
        assert_eq!(mgr.count().unwrap(), 0);
    }

    #[test]
    fn test_search_by_text() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        let mut r1 = make_record("s1", "success");
        r1.processed_text = Some("Rust programming language".to_string());
        let mut r2 = make_record("s2", "success");
        r2.processed_text = Some("Python scripting".to_string());
        mgr.save_transcript(&r1).unwrap();
        mgr.save_transcript(&r2).unwrap();

        let results = mgr.search("Rust", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "s1");

        let results = mgr.search("nonexistent", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_page() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        for i in 0..5 {
            let mut r = make_record(&format!("p{}", i), "success");
            // Use different timestamps to ensure ordering.
            r.timestamp = format!("2025-01-01T00:00:{:02}Z", i);
            mgr.save_transcript(&r).unwrap();
        }

        // First page: 2 records, offset 0.
        let page1 = mgr.get_page(2, 0).unwrap();
        assert_eq!(page1.len(), 2);
        // Most recent first (p4, p3).
        assert_eq!(page1[0].id, "p4");
        assert_eq!(page1[1].id, "p3");

        // Second page: 2 records, offset 2.
        let page2 = mgr.get_page(2, 2).unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].id, "p2");
        assert_eq!(page2[1].id, "p1");

        // Third page: remaining 1 record.
        let page3 = mgr.get_page(2, 4).unwrap();
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].id, "p0");
    }

    #[test]
    fn test_delete_batch() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        for i in 0..5 {
            mgr.save_transcript(&make_record(&format!("b{}", i), "success"))
                .unwrap();
        }
        assert_eq!(mgr.count().unwrap(), 5);

        let deleted = mgr
            .delete_batch(&["b1".to_string(), "b3".to_string()])
            .unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(mgr.count().unwrap(), 3);

        // Deleting empty batch should be a no-op.
        let deleted = mgr.delete_batch(&[]).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_clear_all() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        for i in 0..3 {
            mgr.save_transcript(&make_record(&format!("c{}", i), "success"))
                .unwrap();
        }
        assert_eq!(mgr.count().unwrap(), 3);

        let deleted = mgr.clear_all().unwrap();
        assert_eq!(deleted, 3);
        assert_eq!(mgr.count().unwrap(), 0);
    }
}
