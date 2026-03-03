/**
 * 历史记录数据 hook。
 *
 * 封装分页加载、防抖搜索、删除、清空、重试操作。
 * 从 HistoryPanel.tsx 迁移核心逻辑。
 */
import {
  useState,
  useEffect,
  useCallback,
  useRef,
  type Dispatch,
  type SetStateAction,
} from "react";
import type { TranscriptRecord } from "../../../shared/lib/types";
import { useUIStore } from "../../../shared/stores/uiStore";
import { createLogger } from "../../../shared/lib/logger";

const log = createLogger("useHistory");

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
  const recordsRef = useRef<TranscriptRecord[]>([]);

  useEffect(() => {
    recordsRef.current = records;
  }, [records]);

  useEffect(() => () => clearTimeout(searchTimerRef.current), []);

  const { loadPage, loadMore } = useHistoryPaging(
    recordsRef,
    setRecords,
    setHasMore,
    setIsLoading,
  );
  const setSearchQuery = useHistorySearch(
    loadPage,
    searchTimerRef,
    setSearchQueryState,
    setRecords,
    setHasMore,
  );
  const { deleteRecord, clearAll } = useHistoryMutations(setRecords, setHasMore);
  useInitialHistoryLoad(loadPage);

  return { records, isLoading, hasMore, searchQuery, setSearchQuery, loadMore, deleteRecord, clearAll };
}

function useHistoryPaging(
  recordsRef: { current: TranscriptRecord[] },
  setRecords: Dispatch<SetStateAction<TranscriptRecord[]>>,
  setHasMore: (hasMore: boolean) => void,
  setIsLoading: (loading: boolean) => void,
) {
  const loadPage = useCallback(async (reset: boolean) => {
    const page = await fetchHistoryPage(reset, recordsRef.current.length);
    if (!page) {
      setIsLoading(false);
      return;
    }
    setRecords(reset ? page : (prev) => [...prev, ...page]);
    setHasMore(page.length === PAGE_SIZE);
    setIsLoading(false);
  }, [recordsRef, setHasMore, setIsLoading, setRecords]);

  const loadMore = useCallback(() => loadPage(false), [loadPage]);
  return { loadPage, loadMore };
}

function useHistorySearch(
  loadPage: (reset: boolean) => Promise<void>,
  searchTimerRef: { current: ReturnType<typeof setTimeout> | undefined },
  setSearchQueryState: (value: string) => void,
  setRecords: (records: TranscriptRecord[]) => void,
  setHasMore: (hasMore: boolean) => void,
) {
  return useCallback((value: string) => {
    setSearchQueryState(value);
    clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => {
      void runSearch(value, loadPage, setRecords, setHasMore);
    }, SEARCH_DEBOUNCE_MS);
  }, [loadPage, searchTimerRef, setHasMore, setRecords, setSearchQueryState]);
}

function useHistoryMutations(
  setRecords: Dispatch<SetStateAction<TranscriptRecord[]>>,
  setHasMore: (hasMore: boolean) => void,
) {
  const deleteRecord = useCallback(async (id: string) => {
    if (await invokeHistoryAction("delete_history", { id }, "删除记录失败")) {
      setRecords((prev) => prev.filter((r) => r.id !== id));
    }
  }, [setRecords]);

  const clearAll = useCallback(async () => {
    if (await invokeHistoryAction("clear_history", undefined, "清空记录失败")) {
      setRecords([]);
      setHasMore(false);
    }
  }, [setHasMore, setRecords]);

  return { deleteRecord, clearAll };
}

function useInitialHistoryLoad(loadPage: (reset: boolean) => Promise<void>) {
  useEffect(() => {
    void loadPage(true);
  }, [loadPage]);
}

async function fetchHistoryPage(
  reset: boolean,
  currentCount: number,
): Promise<TranscriptRecord[] | null> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<TranscriptRecord[]>("get_history_page", {
      limit: PAGE_SIZE,
      offset: reset ? 0 : currentCount,
    });
  } catch (e) {
    log.error("[useHistory] 加载历史记录失败:", e);
    useUIStore.getState().showToast({ type: "error", title: "加载历史记录失败" });
    return null;
  }
}

async function runSearch(
  rawQuery: string,
  loadPage: (reset: boolean) => Promise<void>,
  setRecords: (records: TranscriptRecord[]) => void,
  setHasMore: (hasMore: boolean) => void,
) {
  const query = rawQuery.trim();
  if (!query) {
    await loadPage(true);
    return;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const results = await invoke<TranscriptRecord[]>("search_history", {
      query,
      limit: PAGE_SIZE,
    });
    setRecords(results);
    setHasMore(false);
  } catch (e) {
    log.error("[useHistory] 搜索失败:", e);
    useUIStore.getState().showToast({ type: "error", title: "搜索失败" });
  }
}

async function invokeHistoryAction(
  command: "delete_history" | "clear_history",
  payload: Record<string, unknown> | undefined,
  errorTitle: string,
): Promise<boolean> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke(command, payload);
    return true;
  } catch (e) {
    log.error(`[useHistory] ${errorTitle}:`, e);
    useUIStore.getState().showToast({ type: "error", title: errorTitle });
    return false;
  }
}
