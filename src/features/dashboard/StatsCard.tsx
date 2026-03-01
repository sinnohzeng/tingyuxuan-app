/**
 * 统计卡片 — 单个指标展示，增强层次感。
 */
import { Card, Text } from "@fluentui/react-components";

interface StatsCardProps {
  icon: React.ReactElement;
  label: string;
  value: string;
  subtitle?: string;
}

export default function StatsCard({ icon, label, value, subtitle }: StatsCardProps) {
  return (
    <Card
      role="status"
      aria-live="polite"
      className="p-4"
    >
      <div className="flex items-start gap-3">
        <div className="w-10 h-10 rounded-xl bg-gray-50 dark:bg-gray-800 flex items-center justify-center shrink-0">
          {icon}
        </div>
        <div className="flex flex-col gap-0.5 min-w-0">
          <Text size={200} className="text-gray-500 dark:text-gray-400">{label}</Text>
          <Text size={500} weight="bold" className="truncate">{value}</Text>
          {subtitle && (
            <Text size={100} className="text-gray-400 dark:text-gray-500">{subtitle}</Text>
          )}
        </div>
      </div>
    </Card>
  );
}
