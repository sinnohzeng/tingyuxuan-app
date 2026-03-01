/**
 * 首页仪表盘 — 统计概览 + 最近转录。
 */
import { Title3, Text, Spinner, Button } from "@fluentui/react-components";
import { MicRegular, KeyboardRegular } from "@fluentui/react-icons";
import { useStats } from "./hooks/useStats";
import StatsGrid from "./StatsGrid";
import RecentTranscripts from "./RecentTranscripts";

export default function HomePage() {
  const { stats, isLoading } = useStats();

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spinner size="medium" label="加载统计中…" />
      </div>
    );
  }

  // 空状态：尚无使用记录
  if (!stats || stats.total_sessions === 0) {
    return <EmptyState />;
  }

  return (
    <div className="flex flex-col gap-6 p-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <Title3>概览</Title3>
      </div>
      <StatsGrid stats={stats} />
      <RecentTranscripts />
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center gap-6 h-full text-center p-8">
      <div className="w-20 h-20 rounded-2xl bg-blue-50 dark:bg-blue-950 flex items-center justify-center">
        <MicRegular className="text-4xl text-blue-500" />
      </div>
      <div className="flex flex-col gap-2 max-w-sm">
        <Title3>欢迎使用听语轩</Title3>
        <Text className="text-gray-500 dark:text-gray-400">
          按下快捷键开始说话，松开即可输入文字。您的使用统计将在这里展示。
        </Text>
      </div>
      <div className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-gray-100 dark:bg-gray-800">
        <KeyboardRegular className="text-gray-400" />
        <Text size={200} className="text-gray-500 dark:text-gray-400">
          按 <code className="px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded text-xs font-semibold">RAlt</code> 开始听写
        </Text>
      </div>
      <Button appearance="primary" size="large">
        了解快捷键
      </Button>
    </div>
  );
}
