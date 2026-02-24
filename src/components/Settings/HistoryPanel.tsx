import { useState, useEffect, useCallback, useRef } from "react";
import type { TranscriptRecord } from "../../lib/types";

const PAGE_SIZE = 20;

const MODE_LABELS: Record<string, string> = {
  dictate: "听写",
  translate: "翻译",
  ai_assistant: "AI 助手",
  edit: "语音编辑",
};

const STATUS_LABELS: Record<string, { text: string; color: string }> = {
  success: { text: "成功", color: "text-green-600" },
  failed: { text: "失败", color: "text-red-500" },
  queued: { text: "排队中", color: "text-yellow-600" },
  recording: { text: "录音中", color: "text-blue-500" },
  processing: { text: "处理中", color: "text-blue-500" },
  cancelled: { text: "已取消", color: "text-gray-400" },
};

export default function HistoryPanel() {
  const [records, setRecords] = useState<TranscriptRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [copyFeedback, setCopyFeedback] = useState<string | null>(null);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load initial page.
  const loadRecords = useCallback(async (reset = true) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const offset = reset ? 0 : records.length;
      const page = await invoke<TranscriptRecord[]>("get_history_page", {
        limit: PAGE_SIZE,
        offset,
      });
      if (reset) {
        setRecords(page);
      } else {
        setRecords((prev) => [...prev, ...page]);
      }
      setHasMore(page.length === PAGE_SIZE);
    } catch {
      // Dev mode
    }
    setLoading(false);
  }, [records.length]);

  useEffect(() => {
    loadRecords(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Debounced search.
  const handleSearchChange = useCallback(
    (value: string) => {
      setSearchQuery(value);
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
      searchTimerRef.current = setTimeout(async () => {
        if (!value.trim()) {
          loadRecords(true);
          return;
        }
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const results = await invoke<TranscriptRecord[]>("search_history", {
            query: value.trim(),
            limit: PAGE_SIZE,
          });
          setRecords(results);
          setHasMore(false);
        } catch {
          // Dev mode
        }
      }, 300);
    },
    [loadRecords]
  );

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("delete_history", { id });
        setRecords((prev) => prev.filter((r) => r.id !== id));
      } catch {
        // Dev mode
      }
    },
    []
  );

  const handleClearAll = useCallback(async () => {
    if (!window.confirm("确定要清空所有历史记录吗？此操作不可撤销。")) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("clear_history");
      setRecords([]);
      setHasMore(false);
    } catch {
      // Dev mode
    }
  }, []);

  const handleCopy = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopyFeedback("已复制");
      setTimeout(() => setCopyFeedback(null), 1000);
    } catch {
      // Fallback
    }
  }, []);

  const handleRetry = useCallback(
    async (id: string) => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("retry_transcription", { id });
        // Optimistic update: set status to "processing".
        setRecords((prev) =>
          prev.map((r) => (r.id === id ? { ...r, status: "processing" } : r))
        );
      } catch {
        // Dev mode
      }
    },
    []
  );

  const formatTime = (timestamp: string): string => {
    try {
      const d = new Date(timestamp);
      const now = new Date();
      const isToday = d.toDateString() === now.toDateString();
      const time = d.toLocaleTimeString("zh-CN", {
        hour: "2-digit",
        minute: "2-digit",
      });
      if (isToday) return time;
      return `${d.toLocaleDateString("zh-CN", { month: "short", day: "numeric" })} ${time}`;
    } catch {
      return timestamp;
    }
  };

  if (loading) {
    return <div className="text-gray-400 text-sm">加载历史记录...</div>;
  }

  return (
    <div className="space-y-4 max-w-2xl">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-base font-medium text-gray-700">历史记录</h2>
        {records.length > 0 && (
          <button
            onClick={handleClearAll}
            className="text-xs text-red-500 hover:text-red-600 transition-colors"
          >
            清空全部
          </button>
        )}
      </div>

      {/* Search */}
      <input
        type="text"
        value={searchQuery}
        onChange={(e) => handleSearchChange(e.target.value)}
        placeholder="搜索内容..."
        className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm
                   focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
      />

      {/* Copy feedback toast */}
      {copyFeedback && (
        <div className="fixed top-4 right-4 bg-gray-800 text-white text-sm px-3 py-1.5 rounded-lg z-50">
          {copyFeedback}
        </div>
      )}

      {/* Records list */}
      {records.length === 0 ? (
        <div className="text-sm text-gray-400 text-center py-12 border border-dashed border-gray-200 rounded-lg">
          {searchQuery ? "未找到匹配的记录" : "暂无历史记录"}
        </div>
      ) : (
        <div className="space-y-2">
          {records.map((record) => {
            const status = STATUS_LABELS[record.status] || {
              text: record.status,
              color: "text-gray-500",
            };
            const displayText =
              record.processed_text || record.raw_text || null;
            const canRetry =
              record.status === "failed" && !!record.audio_path;

            return (
              <div
                key={record.id}
                className="border border-gray-200 rounded-lg p-3 space-y-2"
              >
                {/* Meta row */}
                <div className="flex items-center gap-2 text-xs">
                  <span className="text-gray-500">
                    {formatTime(record.timestamp)}
                  </span>
                  <span className="bg-gray-100 text-gray-600 px-1.5 py-0.5 rounded">
                    {MODE_LABELS[record.mode] || record.mode}
                  </span>
                  <span className={status.color}>{status.text}</span>
                  {record.error_message && (
                    <span className="text-red-400 truncate max-w-[200px]">
                      {record.error_message}
                    </span>
                  )}
                </div>

                {/* Content */}
                {displayText && (
                  <p className="text-sm text-gray-700 line-clamp-3">
                    {displayText}
                  </p>
                )}

                {/* Action buttons */}
                <div className="flex items-center gap-2 justify-end">
                  {canRetry && (
                    <button
                      onClick={() => handleRetry(record.id)}
                      className="text-xs text-blue-500 hover:text-blue-600 transition-colors"
                    >
                      重试
                    </button>
                  )}
                  {displayText && (
                    <button
                      onClick={() => handleCopy(displayText)}
                      className="text-xs text-gray-500 hover:text-gray-700 transition-colors"
                    >
                      复制
                    </button>
                  )}
                  <button
                    onClick={() => handleDelete(record.id)}
                    className="text-xs text-gray-400 hover:text-red-500 transition-colors"
                  >
                    删除
                  </button>
                </div>
              </div>
            );
          })}

          {/* Load more */}
          {hasMore && (
            <button
              onClick={() => loadRecords(false)}
              className="w-full py-2 text-sm text-blue-500 hover:text-blue-600 transition-colors"
            >
              加载更多
            </button>
          )}
        </div>
      )}
    </div>
  );
}
