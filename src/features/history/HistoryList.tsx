/**
 * 历史记录列表 — 列表渲染 + 加载更多 + 空状态。
 */
import { Button, Text } from "@fluentui/react-components";
import { HistoryRegular } from "@fluentui/react-icons";
import type { TranscriptRecord } from "../../shared/lib/types";
import HistoryItem from "./HistoryItem";

interface HistoryListProps {
  records: TranscriptRecord[];
  hasMore: boolean;
  onLoadMore: () => void;
  onDelete: (id: string) => void;
  onCopy: (text: string) => void;
}

export default function HistoryList({
  records, hasMore, onLoadMore, onDelete, onCopy,
}: HistoryListProps) {
  if (records.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-12">
        <HistoryRegular className="text-4xl text-gray-300" />
        <Text>暂无历史记录</Text>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {records.map((r) => (
        <HistoryItem
          key={r.id}
          record={r}
          onDelete={onDelete}
          onCopy={onCopy}
        />
      ))}
      {hasMore && (
        <Button appearance="secondary" className="self-center" onClick={onLoadMore}>
          加载更多
        </Button>
      )}
    </div>
  );
}
