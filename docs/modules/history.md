# History

## 模块职责

历史记录模块负责语音处理结果的本地持久化，使用 SQLite 存储并提供查询、分页、搜索、删除和统计聚合能力。

**源文件:** `crates/tingyuxuan-core/src/history.rs`

---

## 关键类型定义

### TranscriptRecord

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    pub id: String,
    pub timestamp: String,
    pub mode: String,
    pub raw_text: Option<String>,
    pub processed_text: Option<String>,
    pub status: String,
    pub context_json: Option<String>,
    pub duration_ms: Option<i64>,
    pub language: Option<String>,
    pub error_message: Option<String>,
}
```

- `context_json` 为 InputContext 的 JSON 字符串，替代旧 `app_context` 纯文本字段。
- `raw_text` 为可选字段，主流程以 `processed_text` 为最终输出。

### AggregateStats

```rust
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
```

### HistoryManager

```rust
pub struct HistoryManager {
    conn: Connection,
}
```

---

## Public API

### 构造与初始化

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new() -> Result<Self, HistoryError>` | 打开文件数据库并初始化表/索引 |
| `new_in_memory()` | `fn new_in_memory() -> Result<Self, HistoryError>` | 仅测试使用，内存数据库 |

### 写入与状态更新

| 方法 | 签名 | 说明 |
|------|------|------|
| `save_transcript()` | `fn save_transcript(&self, record: &TranscriptRecord) -> Result<(), HistoryError>` | UPSERT 保存记录 |
| `update_status()` | `fn update_status(&self, id: &str, status: &str) -> Result<(), HistoryError>` | 更新状态 |
| `update_processed()` | `fn update_processed(&self, id: &str, raw_text: &str, processed_text: &str) -> Result<(), HistoryError>` | 同时更新 raw/processed 并置为 success |
| `update_result()` | `fn update_result(&self, id: &str, processed_text: &str) -> Result<(), HistoryError>` | 仅更新 processed 并置为 success |

### 查询

| 方法 | 签名 | 说明 |
|------|------|------|
| `get_recent()` | `fn get_recent(&self, limit: u32) -> Result<Vec<TranscriptRecord>, HistoryError>` | 最近记录 |
| `get_by_id()` | `fn get_by_id(&self, id: &str) -> Result<Option<TranscriptRecord>, HistoryError>` | 按 ID 查询 |
| `search()` | `fn search(&self, query: &str, limit: u32) -> Result<Vec<TranscriptRecord>, HistoryError>` | `raw_text` / `processed_text` LIKE 搜索 |
| `get_page()` | `fn get_page(&self, limit: u32, offset: u32) -> Result<Vec<TranscriptRecord>, HistoryError>` | 分页查询 |
| `count()` | `fn count(&self) -> Result<u64, HistoryError>` | 总记录数 |
| `get_stats()` | `fn get_stats(&self) -> Result<AggregateStats, HistoryError>` | 聚合统计 |
| `get_dictionary_utilization()` | `fn get_dictionary_utilization(&self, dictionary: &[String]) -> Result<f64, HistoryError>` | 词典命中率 |

### 删除

| 方法 | 签名 | 说明 |
|------|------|------|
| `delete()` | `fn delete(&self, id: &str) -> Result<(), HistoryError>` | 删除单条 |
| `delete_batch()` | `fn delete_batch(&self, ids: &[String]) -> Result<u64, HistoryError>` | 批量删除并返回删除数 |
| `clear_all()` | `fn clear_all(&self) -> Result<u64, HistoryError>` | 清空全部并返回删除数 |

---

## 数据库 Schema

```sql
CREATE TABLE IF NOT EXISTS transcripts (
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
CREATE INDEX IF NOT EXISTS idx_transcripts_timestamp ON transcripts(timestamp DESC);
```

- `audio_path` 为历史兼容列，当前 Rust `TranscriptRecord` 不直接映射该列。
- 存储路径：`{data_dir}/history/transcripts.db`

---

## 迁移策略

- 初始化时执行 `migrate_app_context()`：
  - 检测旧列 `app_context` 是否存在。
  - 不存在 `context_json` 时先 `ALTER TABLE` 添加列。
  - 将旧值迁移为 `json_object('app_name', app_context)`。
- 迁移失败为 best-effort，仅记录告警，不阻断主流程。

---

## 错误处理

`HistoryError` 统一封装数据库与 IO 错误：

```rust
pub enum HistoryError {
    DatabaseError(#[from] rusqlite::Error),
    IoError(#[from] std::io::Error),
}
```

---

## 已知限制

1. 搜索采用 `LIKE %query%`，大数据量下性能有限，未使用 FTS5。
2. 单连接 + `Mutex` 串行访问，高并发写入场景有上限。
3. 删除历史记录时不会自动清理潜在的旧 `audio_path` 关联文件。
4. `created_at` 存在于表中，但未在 `TranscriptRecord` 暴露。
