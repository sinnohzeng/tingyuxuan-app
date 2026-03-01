/**
 * 首页仪表盘 — 统计概览 + 最近转录。
 */
import { Title3, Text, Spinner } from "@fluentui/react-components";
import { MicRegular } from "@fluentui/react-icons";
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
    <div className="flex flex-col gap-6 p-6">
      <Title3>概览</Title3>
      <StatsGrid stats={stats} />
      <RecentTranscripts />
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center gap-4 h-full text-center p-8">
      <MicRegular className="text-5xl text-gray-300" />
      <Title3>欢迎使用听语轩</Title3>
      <Text>
        按住快捷键开始说话，松开即可输入文字。
        <br />
        您的使用统计将在这里展示。
      </Text>
    </div>
  );
}
