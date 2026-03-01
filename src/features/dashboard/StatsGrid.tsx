/**
 * 统计网格 — 渲染 5 个 StatsCard。
 */
import {
  TimerRegular,
  TextWordCountRegular,
  TopSpeedRegular,
  SparkleRegular,
  ClockRegular,
} from "@fluentui/react-icons";
import type { DashboardStats } from "../../shared/lib/types";
import StatsCard from "./StatsCard";

interface StatsGridProps {
  stats: DashboardStats;
}

function formatDuration(ms: number): string {
  const hours = Math.floor(ms / 3_600_000);
  const minutes = Math.round((ms % 3_600_000) / 60_000);
  if (hours > 0) return `${hours} 小时 ${minutes} 分`;
  return `${minutes} 分钟`;
}

export default function StatsGrid({ stats }: StatsGridProps) {
  const cards = [
    {
      icon: <SparkleRegular className="text-2xl text-purple-500" />,
      label: "个性化",
      value: `${Math.round(stats.dictionary_utilization * 100)}%`,
      subtitle: "词典利用率",
    },
    {
      icon: <TimerRegular className="text-2xl text-blue-500" />,
      label: "总录音时间",
      value: formatDuration(stats.total_duration_ms),
      subtitle: `${stats.successful_sessions} 次成功`,
    },
    {
      icon: <TextWordCountRegular className="text-2xl text-green-500" />,
      label: "总字数",
      value: stats.total_char_count.toLocaleString(),
    },
    {
      icon: <ClockRegular className="text-2xl text-orange-500" />,
      label: "节省时间",
      value: formatDuration(stats.estimated_time_saved_ms),
    },
    {
      icon: <TopSpeedRegular className="text-2xl text-red-500" />,
      label: "平均速度",
      value: `${Math.round(stats.average_speed_cpm)} 字/分`,
    },
  ];

  return (
    <div className="grid grid-cols-2 gap-4 lg:grid-cols-3">
      {cards.map((card) => (
        <StatsCard key={card.label} {...card} />
      ))}
    </div>
  );
}
