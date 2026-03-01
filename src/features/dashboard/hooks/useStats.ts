/**
 * 仪表盘统计 hook。
 *
 * 薄 wrapper：mount 时触发 statsStore.fetchStats()，返回消费侧数据。
 */
import { useEffect } from "react";
import { useStatsStore } from "../../../shared/stores/statsStore";
import type { DashboardStats } from "../../../shared/lib/types";

export interface UseStatsReturn {
  stats: DashboardStats | null;
  isLoading: boolean;
  error: string | null;
}

export function useStats(): UseStatsReturn {
  const stats = useStatsStore((s) => s.stats);
  const isLoading = useStatsStore((s) => s.isLoading);
  const error = useStatsStore((s) => s.error);
  const fetchStats = useStatsStore((s) => s.fetchStats);

  useEffect(() => {
    fetchStats();
  }, [fetchStats]);

  return { stats, isLoading, error };
}
