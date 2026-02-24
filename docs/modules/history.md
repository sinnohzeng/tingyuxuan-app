# History

## 模块职责

历史记录模块负责存储和管理语音转录记录，使用 SQLite 作为持久化存储。提供 CRUD 操作、搜索、分页查询和批量删除等功能。

**源文件:** `crates/tingyuxuan-core/src/history.rs`

---

## 关键类型定义

### TranscriptRecord

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    pub id: String,                    // UUID 主键
    pub timestamp: String,             // RFC 3339 时间戳
    pub mode: String,                  // 处理模式: "dictate" | "translate" | "ai_assistant" | "edit"
    pub raw_text: Option<String>,      // STT 原始转录文本
    pub processed_text: Option<String>,// LLM 处理后文本
    pub audio_path: Option<String>,    // 音频文件路径
    pub status: String,                // 记录状态（见下方状态表）
    pub app_context: Option<String>,   // 录音时的活动窗口名称
    pub duration_ms: Option<i64>,      // 录音时长（毫秒）
    pub language: Option<String>,      // 检测到的语言
    pub error_message: Option<String>, // 失败时的错误信息
}
```

### HistoryManager

```rust
pub struct HistoryManager {
    conn: Connection,  // rusqlite::Connection
}
```

封装 SQLite 连接，提供所有历史记录操作方法。非 `Clone`，通过 `Arc<Mutex<HistoryManager>>` 在 Tauri state 中共享。

### 记录状态值

| 状态 | 含义 |
|------|------|
| `"recording"` | 正在录音 |
| `"pending"` | 等待处理 |
| `"processing"` | 正在处理中（STT/LLM） |
| `"success"` | 处理完成 |
| `"failed"` | 处理失败 |
| `"cancelled"` | 用户取消 |
| `"queued"` | 离线排队等待网络恢复 |

---

## Public API

### 构造函数

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new() -> Result<Self, HistoryError>` | 在 `data_dir/history/transcripts.db` 创建/打开 SQLite 数据库，执行 `CREATE TABLE IF NOT EXISTS` 和索引创建 |
| `new_in_memory()` | `fn new_in_memory() -> Result<Self, HistoryError>` | 仅测试用 (`#[cfg(test)]`)，使用内存数据库 |

### 写入操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `save_transcript(&self, record)` | `fn save_transcript(&self, record: &TranscriptRecord) -> Result<(), HistoryError>` | 插入或替换记录（`INSERT OR REPLACE`） |
| `update_status(&self, id, status)` | `fn update_status(&self, id: &str, status: &str) -> Result<(), HistoryError>` | 更新记录状态字段 |
| `update_processed(&self, id, raw_text, processed_text)` | `fn update_processed(&self, id: &str, raw_text: &str, processed_text: &str) -> Result<(), HistoryError>` | 更新 raw_text 和 processed_text，状态设为 `"success"` |
| `update_result(&self, id, processed_text)` | `fn update_result(&self, id: &str, processed_text: &str) -> Result<(), HistoryError>` | 仅更新 processed_text 并标记为 `"success"`（当 raw_text 不可用时使用） |

### 查询操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `get_recent(&self, limit)` | `fn get_recent(&self, limit: u32) -> Result<Vec<TranscriptRecord>, HistoryError>` | 获取最近 N 条记录（按 timestamp DESC） |
| `get_by_id(&self, id)` | `fn get_by_id(&self, id: &str) -> Result<Option<TranscriptRecord>, HistoryError>` | 按 ID 查询单条记录 |
| `get_pending(&self)` | `fn get_pending() -> Result<Vec<TranscriptRecord>, HistoryError>` | 获取所有 `pending` 或 `failed` 状态的记录 |
| `search(&self, query, limit)` | `fn search(&self, query: &str, limit: u32) -> Result<Vec<TranscriptRecord>, HistoryError>` | 在 raw_text 和 processed_text 中搜索（`LIKE %query%`） |
| `get_page(&self, limit, offset)` | `fn get_page(&self, limit: u32, offset: u32) -> Result<Vec<TranscriptRecord>, HistoryError>` | 分页查询（`LIMIT ?1 OFFSET ?2`，按 timestamp DESC） |
| `count(&self)` | `fn count(&self) -> Result<u64, HistoryError>` | 返回记录总数 |

### 删除操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `delete(&self, id)` | `fn delete(&self, id: &str) -> Result<(), HistoryError>` | 删除单条记录 |
| `delete_batch(&self, ids)` | `fn delete_batch(&self, ids: &[String]) -> Result<u64, HistoryError>` | 批量删除。使用参数化 `IN` 子句（`?1, ?2, ...`）防止 SQL 注入。空列表时返回 `Ok(0)` |
| `clear_all(&self)` | `fn clear_all(&self) -> Result<u64, HistoryError>` | 删除所有记录，返回删除行数 |

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
    app_context TEXT,
    duration_ms INTEGER,
    language TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_transcripts_status ON transcripts(status);
CREATE INDEX IF NOT EXISTS idx_transcripts_timestamp ON transcripts(timestamp DESC);
```

**存储位置:** `~/.local/share/tingyuxuan/TingYuXuan/history/transcripts.db`

---

## 错误处理策略

使用自定义错误类型 `HistoryError`（定义在 `error.rs`）：

```rust
pub enum HistoryError {
    DatabaseError(#[from] rusqlite::Error),  // SQLite 操作失败
    IoError(#[from] std::io::Error),          // 文件系统错误（创建目录等）
}
```

- `new()` 在无法确定数据目录时将 `ConfigError` 转换为 `IoError` 返回
- 所有数据库操作错误通过 `rusqlite::Error` 自动转换为 `HistoryError::DatabaseError`
- Tauri command 层（`commands.rs`）将 `HistoryError` 转为 `String` 返回给前端
- `save_transcript` 使用 `INSERT OR REPLACE`，对相同 ID 的记录执行 upsert 而非报错

---

## 测试覆盖

共 **10 个单元测试**，全部使用 `new_in_memory()` 内存数据库：

| 测试 | 覆盖内容 |
|------|---------|
| `test_create_and_query` | 插入记录后通过 `get_recent` 查询验证 |
| `test_get_pending` | 验证按状态 (pending/failed) 过滤查询 |
| `test_update_status` | 更新状态后验证新状态值 |
| `test_get_by_id_found` | 按 ID 查询存在的记录 |
| `test_get_by_id_not_found` | 按 ID 查询不存在的记录返回 `None` |
| `test_delete` | 删除单条记录后验证 count 为 0 |
| `test_search_by_text` | 搜索 processed_text 中的关键词，验证匹配和不匹配 |
| `test_get_page` | 5 条记录的分页查询（page size=2），验证排序和 offset |
| `test_delete_batch` | 批量删除 2 条记录，验证剩余 3 条；空批量返回 0 |
| `test_clear_all` | 清空所有记录，验证返回删除数和最终 count |

---

## 已知局限性

1. **无全文搜索索引**: `search()` 使用 `LIKE %query%`，无法利用索引，大数据量下性能较差。未使用 SQLite FTS5 扩展
2. **无数据导出/导入**: 没有 CSV/JSON 导出功能
3. **无自动清理**: 没有基于时间或数量自动清理旧记录的机制。`CacheConfig` 中的 `audio_retention_hours` 和 `failed_retention_days` 尚未在 history 模块中实现
4. **无 Schema 迁移**: 表结构变更时没有版本化迁移机制，仅依赖 `CREATE TABLE IF NOT EXISTS`
5. **单连接模型**: `HistoryManager` 持有单个 `Connection`，通过 `Mutex` 串行化访问。高并发写入场景下可能成为瓶颈
6. **timestamp 为 String**: 时间戳存储为 RFC 3339 字符串而非 SQLite 的 INTEGER (Unix epoch)，范围查询需要字符串比较
7. **无音频文件关联清理**: 删除历史记录时不会同时删除关联的音频文件
8. **`created_at` 列未暴露**: 数据库中有 `created_at` 列（默认为 `datetime('now')`），但 `TranscriptRecord` 结构体中未包含该字段
