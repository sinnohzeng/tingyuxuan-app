/**
 * 历史记录页面 — 搜索 + 列表 + 批量操作。
 */
import { useCallback } from "react";
import { Title3, SearchBox, Button, Spinner, Text } from "@fluentui/react-components";
import { DeleteRegular } from "@fluentui/react-icons";
import { useHistory } from "./hooks/useHistory";
import HistoryList from "./HistoryList";

export default function HistoryPage() {
  const {
    records, isLoading, hasMore, searchQuery,
    setSearchQuery, loadMore, deleteRecord, clearAll,
  } = useHistory();

  const handleCopy = useCallback((text: string) => {
    navigator.clipboard.writeText(text).catch(() => {});
  }, []);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spinner size="medium" label="加载历史记录…" />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 p-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Title3>历史记录</Title3>
          {records.length > 0 && (
            <Text size={200} className="text-gray-400 dark:text-gray-500 tabular-nums">
              {records.length} 条
            </Text>
          )}
        </div>
        {records.length > 0 && (
          <Button
            appearance="subtle"
            size="small"
            icon={<DeleteRegular />}
            onClick={clearAll}
          >
            清空
          </Button>
        )}
      </div>

      <SearchBox
        placeholder="搜索历史记录…"
        value={searchQuery}
        onChange={(_, data) => setSearchQuery(data.value)}
        className="max-w-sm"
      />

      <HistoryList
        records={records}
        hasMore={hasMore}
        onLoadMore={loadMore}
        onDelete={deleteRecord}
        onCopy={handleCopy}
      />
    </div>
  );
}
