use crate::config::AppConfig;
use crate::error::HistoryError;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

/// A single transcript record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    pub id: String,
    pub timestamp: String,
    pub mode: String,
    pub raw_text: Option<String>,
    pub processed_text: Option<String>,
    pub status: String,
    /// JSON 序列化的 InputContext，替代旧的 app_context 字符串
    pub context_json: Option<String>,
    pub duration_ms: Option<i64>,
    pub language: Option<String>,
    pub error_message: Option<String>,
}

/// 聚合统计数据（仪表盘用）。
///
/// 通过单次 SQL 聚合查询计算，前端 statsStore 提供 60 秒 TTL 缓存。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateStats {
    pub total_sessions: u64,
    pub successful_sessions: u64,
    pub total_duration_ms: u64,
    pub total_char_count: u64,
    pub dictionary_utilization: f64,
    pub average_speed_cpm: f64,
    pub estimated_time_saved_ms: u64,
}

pub struct HistoryManager {
    conn: Connection,
}

impl HistoryManager {
    /// Create a new HistoryManager with a file-based SQLite database.
    pub fn new() -> Result<Self, HistoryError> {
        let data_dir = AppConfig::data_dir().map_err(|e| {
            HistoryError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                e.to_string(),
            ))
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
                context_json TEXT,
                duration_ms INTEGER,
                language TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_transcripts_status ON transcripts(status);
            CREATE INDEX IF NOT EXISTS idx_transcripts_timestamp ON transcripts(timestamp DESC);",
        )?;
        // 迁移旧表：如果 app_context 列存在，将数据迁移到 context_json
        self.migrate_app_context();
        Ok(())
    }

    /// 将旧的 app_context 列数据迁移到 context_json 列。
    /// 旧数据格式为纯应用名称字符串，迁移为 `{"app_name": "..."}` JSON。
    fn migrate_app_context(&self) {
        // 检查旧列是否存在
        let has_old_column = self
            .conn
            .prepare("SELECT app_context FROM transcripts LIMIT 0")
            .is_ok();
        if !has_old_column {
            return;
        }
        // 检查新列是否存在
        let has_new_column = self
            .conn
            .prepare("SELECT context_json FROM transcripts LIMIT 0")
            .is_ok();
        if !has_new_column {
            // 添加新列
            if let Err(e) = self
                .conn
                .execute_batch("ALTER TABLE transcripts ADD COLUMN context_json TEXT;")
            {
                tracing::warn!("Failed to add context_json column: {e}");
                return;
            }
        }
        // 迁移非空 app_context 数据到 context_json（仅当 context_json 为空时）
        if let Err(e) = self.conn.execute(
            "UPDATE transcripts SET context_json = json_object('app_name', app_context) \
             WHERE app_context IS NOT NULL AND app_context != '' AND context_json IS NULL",
            [],
        ) {
            tracing::warn!("Failed to migrate app_context data: {e}");
        }
    }

    /// Save a new transcript record.
    pub fn save_transcript(&self, record: &TranscriptRecord) -> Result<(), HistoryError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO transcripts
             (id, timestamp, mode, raw_text, processed_text,
              status, context_json, duration_ms, language, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.id,
                record.timestamp,
                record.mode,
                record.raw_text,
                record.processed_text,
                record.status,
                record.context_json,
                record.duration_ms,
                record.language,
                record.error_message,
            ],
        )?;
        Ok(())
    }

    /// Column list for SELECT queries (audio_path excluded from Rust mapping).
    const SELECT_COLUMNS: &str = "id, timestamp, mode, raw_text, processed_text, \
         status, context_json, duration_ms, language, error_message";

    /// Map a row (matching SELECT_COLUMNS order) to a TranscriptRecord.
    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<TranscriptRecord> {
        Ok(TranscriptRecord {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            mode: row.get(2)?,
            raw_text: row.get(3)?,
            processed_text: row.get(4)?,
            status: row.get(5)?,
            context_json: row.get(6)?,
            duration_ms: row.get(7)?,
            language: row.get(8)?,
            error_message: row.get(9)?,
        })
    }

    /// Get the most recent transcript records.
    pub fn get_recent(&self, limit: u32) -> Result<Vec<TranscriptRecord>, HistoryError> {
        let sql = format!(
            "SELECT {} FROM transcripts ORDER BY timestamp DESC LIMIT ?1",
            Self::SELECT_COLUMNS
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let records = stmt
            .query_map(params![limit], Self::row_to_record)?
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
        let sql = format!(
            "SELECT {} FROM transcripts WHERE id = ?1",
            Self::SELECT_COLUMNS
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let mut rows = stmt.query_map(params![id], Self::row_to_record)?;

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
        let sql = format!(
            "SELECT {} FROM transcripts \
             WHERE raw_text LIKE ?1 OR processed_text LIKE ?1 \
             ORDER BY timestamp DESC LIMIT ?2",
            Self::SELECT_COLUMNS
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let records = stmt
            .query_map(params![pattern, limit], Self::row_to_record)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Paginated query with LIMIT and OFFSET.
    pub fn get_page(&self, limit: u32, offset: u32) -> Result<Vec<TranscriptRecord>, HistoryError> {
        let sql = format!(
            "SELECT {} FROM transcripts ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
            Self::SELECT_COLUMNS
        );
        let mut stmt = self.conn.prepare(&sql)?;

        let records = stmt
            .query_map(params![limit, offset], Self::row_to_record)?
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
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let deleted = self.conn.execute(&sql, params.as_slice())?;
        Ok(deleted as u64)
    }

    /// Delete all records. Returns the number of deleted rows.
    pub fn clear_all(&self) -> Result<u64, HistoryError> {
        let deleted = self.conn.execute("DELETE FROM transcripts", [])?;
        Ok(deleted as u64)
    }

    /// 聚合统计：总会话数、成功数、时长、字数 + 派生指标。
    ///
    /// 单次 SQL 查询，全列 COALESCE 防止空表 NULL。
    pub fn get_stats(&self) -> Result<AggregateStats, HistoryError> {
        let mut stats = self.conn.query_row(
            "SELECT
                COUNT(*) AS total,
                COALESCE(SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'success' THEN duration_ms ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'success' THEN LENGTH(processed_text) ELSE 0 END), 0)
             FROM transcripts",
            [],
            |row| {
                // SQLite 内部使用 i64，转换为 u64
                Ok(AggregateStats {
                    total_sessions: row.get::<_, i64>(0)? as u64,
                    successful_sessions: row.get::<_, i64>(1)? as u64,
                    total_duration_ms: row.get::<_, i64>(2)? as u64,
                    total_char_count: row.get::<_, i64>(3)? as u64,
                    ..Default::default()
                })
            },
        )?;

        // 派生指标：平均速度（字/分钟）
        if stats.total_duration_ms > 0 {
            stats.average_speed_cpm =
                (stats.total_char_count as f64) / (stats.total_duration_ms as f64 / 60_000.0);
        }
        // 估算节省时间：手打 40 字/分钟 vs 语音输入实际耗时
        let manual_time_ms = (stats.total_char_count as f64 / 40.0) * 60_000.0;
        stats.estimated_time_saved_ms =
            (manual_time_ms - stats.total_duration_ms as f64).max(0.0) as u64;

        Ok(stats)
    }

    /// 计算词典利用率：成功记录中包含词典词汇的比例。
    pub fn get_dictionary_utilization(&self, dictionary: &[String]) -> Result<f64, HistoryError> {
        if dictionary.is_empty() {
            return Ok(0.0);
        }
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM transcripts WHERE status = 'success'",
            [],
            |r| r.get(0),
        )?;
        if total == 0 {
            return Ok(0.0);
        }

        let conditions: Vec<String> = (1..=dictionary.len())
            .map(|i| format!("processed_text LIKE ?{i}"))
            .collect();
        let sql = format!(
            "SELECT COUNT(*) FROM transcripts WHERE status = 'success' AND ({conditions})",
            conditions = conditions.join(" OR ")
        );
        let patterns: Vec<String> = dictionary.iter().map(|w| format!("%{w}%")).collect();
        let params: Vec<&dyn rusqlite::types::ToSql> = patterns
            .iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();
        let matched: i64 = self.conn.query_row(&sql, params.as_slice(), |r| r.get(0))?;

        Ok(matched as f64 / total as f64)
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
            status: status.to_string(),
            context_json: None,
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

    // --- AggregateStats tests ---

    #[test]
    fn test_stats_empty_database() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        let stats = mgr.get_stats().unwrap();
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.successful_sessions, 0);
        assert_eq!(stats.total_duration_ms, 0);
        assert_eq!(stats.total_char_count, 0);
        assert_eq!(stats.average_speed_cpm, 0.0);
        assert_eq!(stats.estimated_time_saved_ms, 0);
    }

    #[test]
    fn test_stats_counts_correctly() {
        let mgr = HistoryManager::new_in_memory().unwrap();
        mgr.save_transcript(&make_record("s1", "success")).unwrap();
        mgr.save_transcript(&make_record("s2", "success")).unwrap();
        mgr.save_transcript(&make_record("s3", "failed")).unwrap();
        mgr.save_transcript(&make_record("s4", "cancelled"))
            .unwrap();

        let stats = mgr.get_stats().unwrap();
        assert_eq!(stats.total_sessions, 4);
        assert_eq!(stats.successful_sessions, 2);
    }

    #[test]
    fn test_stats_duration_and_chars() {
        let mgr = HistoryManager::new_in_memory().unwrap();

        let mut r1 = make_record("d1", "success");
        r1.duration_ms = Some(5000);
        r1.processed_text = Some("Hello".to_string()); // 5 chars
        mgr.save_transcript(&r1).unwrap();

        let mut r2 = make_record("d2", "success");
        r2.duration_ms = Some(10000);
        r2.processed_text = Some("World!".to_string()); // 6 chars
        mgr.save_transcript(&r2).unwrap();

        // Failed record should not contribute to duration/chars
        let mut r3 = make_record("d3", "failed");
        r3.duration_ms = Some(99999);
        r3.processed_text = Some("Should not count".to_string());
        mgr.save_transcript(&r3).unwrap();

        let stats = mgr.get_stats().unwrap();
        assert_eq!(stats.total_duration_ms, 15000); // 5000 + 10000
        assert_eq!(stats.total_char_count, 11); // 5 + 6
    }

    #[test]
    fn test_stats_speed_and_time_saved() {
        let mgr = HistoryManager::new_in_memory().unwrap();

        let mut r = make_record("sp1", "success");
        r.duration_ms = Some(60_000); // 1 minute
        r.processed_text = Some("A".repeat(120)); // 120 chars
        mgr.save_transcript(&r).unwrap();

        let stats = mgr.get_stats().unwrap();
        // 120 chars / 1 min = 120 CPM
        assert!((stats.average_speed_cpm - 120.0).abs() < 0.01);
        // Manual: 120 / 40 * 60000 = 180_000 ms
        // Saved: 180_000 - 60_000 = 120_000 ms
        assert_eq!(stats.estimated_time_saved_ms, 120_000);
    }

    #[test]
    fn test_dictionary_utilization() {
        let mgr = HistoryManager::new_in_memory().unwrap();

        let mut r1 = make_record("du1", "success");
        r1.processed_text = Some("今天的Rust编程很顺利".to_string());
        mgr.save_transcript(&r1).unwrap();

        let mut r2 = make_record("du2", "success");
        r2.processed_text = Some("Python也不错".to_string());
        mgr.save_transcript(&r2).unwrap();

        let mut r3 = make_record("du3", "success");
        r3.processed_text = Some("普通的一天".to_string());
        mgr.save_transcript(&r3).unwrap();

        // Failed records should not count
        let mut r4 = make_record("du4", "failed");
        r4.processed_text = Some("Rust失败了".to_string());
        mgr.save_transcript(&r4).unwrap();

        let dict = vec!["Rust".to_string()];
        let util = mgr.get_dictionary_utilization(&dict).unwrap();
        // 1 out of 3 success records contains "Rust"
        assert!((util - 1.0 / 3.0).abs() < 0.01);

        // Empty dictionary
        let util = mgr.get_dictionary_utilization(&[]).unwrap();
        assert_eq!(util, 0.0);

        // Multiple words
        let dict = vec!["Rust".to_string(), "Python".to_string()];
        let util = mgr.get_dictionary_utilization(&dict).unwrap();
        // 2 out of 3 success records contain Rust or Python
        assert!((util - 2.0 / 3.0).abs() < 0.01);
    }
}
