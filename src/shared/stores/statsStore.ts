import { create } from "zustand";
import type { DashboardStats } from "../lib/types";
import { createLogger } from "../lib/logger";

const log = createLogger("statsStore");

const CACHE_TTL_MS = 60_000; // 60 秒缓存

interface StatsStore {
  stats: DashboardStats | null;
  lastFetched: number | null;
  isLoading: boolean;
  error: string | null;

  /** 获取统计（带缓存） */
  fetchStats: () => Promise<void>;
  /** 使缓存失效（录音完成后调用） */
  invalidate: () => void;
}

export const useStatsStore = create<StatsStore>((set, get) => ({
  stats: null,
  lastFetched: null,
  isLoading: false,
  error: null,

  fetchStats: async () => {
    const { lastFetched, isLoading } = get();

    // 缓存未过期 → 跳过
    if (lastFetched && Date.now() - lastFetched < CACHE_TTL_MS) return;
    // 正在加载 → 跳过
    if (isLoading) return;

    set({ isLoading: true, error: null });
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const stats = await invoke<DashboardStats>("get_dashboard_stats");
      set({ stats, lastFetched: Date.now(), isLoading: false });
    } catch (e) {
      log.error("获取统计数据失败:", e);
      set({ error: String(e), isLoading: false });
    }
  },

  invalidate: () => set({ lastFetched: null }),
}));
