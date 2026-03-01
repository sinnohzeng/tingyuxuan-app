/**
 * 历史记录数据 hook。
 *
 * 封装分页加载、防抖搜索、删除、清空、重试操作。
 * 从 HistoryPanel.tsx 迁移核心逻辑。
 */
import { useState, useEffect, useCallback, useRef } from "react";
import type { TranscriptRecord } from "../../../shared/lib/types";
import { useUIStore } from "../../../shared/stores/uiStore";

const PAGE_SIZE = 20;
const SEARCH_DEBOUNCE_MS = 300;

export interface UseHistoryReturn {
  records: TranscriptRecord[];
  isLoading: boolean;
  hasMore: boolean;
  searchQuery: string;
  setSearchQuery: (q: string) => void;
  loadMore: () => Promise<void>;
  deleteRecord: (id: string) => Promise<void>;
  clearAll: () => Promise<void>;
}

export function useHistory(): UseHistoryReturn {
  const [records, setRecords] = useState<TranscriptRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);
  const [searchQuery, setSearchQueryState] = useState("");
  const searchTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const recordsRef = useRef(records);
  recordsRef.current = records;

  useEffect(() => () => clearTimeout(searchTimerRef.current), []);

  const loadPage = useCallback(async (reset: boolean) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const offset = reset ? 0 : recordsRef.current.length;
      const page = await invoke<TranscriptRecord[]>("get_history_page", {
        limit: PAGE_SIZE,
        offset,
      });
      setRecords(reset ? page : (prev) => [...prev, ...page]);
      setHasMore(page.length === PAGE_SIZE);
    } catch (e) {
      console.error("[useHistory] 加载历史记录失败:", e);
      useUIStore.getState().showToast({ type: "error", title: "加载历史记录失败" });
    }
    setIsLoading(false);
  }, []);

  useEffect(() => {
    loadPage(true);
  }, [loadPage]);

  const setSearchQuery = useCallback(
    (value: string) => {
      setSearchQueryState(value);
      clearTimeout(searchTimerRef.current);
      searchTimerRef.current = setTimeout(async () => {
        if (!value.trim()) {
          loadPage(true);
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
        } catch (e) {
          console.error("[useHistory] 搜索失败:", e);
          useUIStore.getState().showToast({ type: "error", title: "搜索失败" });
        }
      }, SEARCH_DEBOUNCE_MS);
    },
    [loadPage],
  );

  const loadMore = useCallback(() => loadPage(false), [loadPage]);

  const deleteRecord = useCallback(async (id: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_history", { id });
      setRecords((prev) => prev.filter((r) => r.id !== id));
    } catch (e) {
      console.error("[useHistory] 删除记录失败:", e);
      useUIStore.getState().showToast({ type: "error", title: "删除记录失败" });
    }
  }, []);

  const clearAll = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("clear_history");
      setRecords([]);
      setHasMore(false);
    } catch (e) {
      console.error("[useHistory] 清空记录失败:", e);
      useUIStore.getState().showToast({ type: "error", title: "清空记录失败" });
    }
  }, []);

  return {
    records,
    isLoading,
    hasMore,
    searchQuery,
    setSearchQuery,
    loadMore,
    deleteRecord,
    clearAll,
  };
}
