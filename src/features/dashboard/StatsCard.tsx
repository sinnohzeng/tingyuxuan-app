/**
 * 统计卡片 — 单个指标展示。
 */
import { Card, CardHeader, Text } from "@fluentui/react-components";

interface StatsCardProps {
  icon: React.ReactElement;
  label: string;
  value: string;
  subtitle?: string;
}

export default function StatsCard({ icon, label, value, subtitle }: StatsCardProps) {
  return (
    <Card role="status" aria-live="polite">
      <CardHeader
        image={icon}
        header={<Text weight="semibold">{label}</Text>}
        description={
          <div>
            <Text size={500} weight="bold">{value}</Text>
            {subtitle && (
              <Text size={200} className="block">{subtitle}</Text>
            )}
          </div>
        }
      />
    </Card>
  );
}
