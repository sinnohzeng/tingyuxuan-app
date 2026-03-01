/**
 * 个性化 Tab — 词典利用率 + 隐私说明。
 */
import { Text, Title3, Card, CardHeader } from "@fluentui/react-components";
import { ShieldCheckmarkRegular } from "@fluentui/react-icons";
import { useStatsStore } from "../../../shared/stores/statsStore";
import { useEffect } from "react";

export default function PersonalizationTab() {
  const stats = useStatsStore((s) => s.stats);
  const fetchStats = useStatsStore((s) => s.fetchStats);

  useEffect(() => {
    fetchStats();
  }, [fetchStats]);

  const utilization = stats
    ? `${Math.round(stats.dictionary_utilization * 100)}%`
    : "—";

  return (
    <div className="flex flex-col gap-6 py-4">
      {/* 个性化程度 */}
      <div>
        <Title3>个性化</Title3>
        <Text className="block mt-1" size={200}>
          词典利用率反映了您的个人词典在润色中的使用频率。
        </Text>
      </div>

      <Card>
        <CardHeader
          header={<Text weight="semibold">词典利用率</Text>}
          description={<Text size={500}>{utilization}</Text>}
        />
      </Card>

      {/* 隐私声明 */}
      <Card>
        <CardHeader
          image={<ShieldCheckmarkRegular className="text-2xl text-green-600" />}
          header={<Text weight="semibold">您的数据保持私密</Text>}
          description={
            <Text size={200}>
              所有语音和文本数据仅存储在您的设备上，不会上传到任何服务器。
              API 密钥安全存储在操作系统密钥链中。
            </Text>
          }
        />
      </Card>
    </div>
  );
}
