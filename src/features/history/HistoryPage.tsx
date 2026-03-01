/**
 * 历史记录页面 — 搜索 + 列表 + 批量操作。
 */
import { useCallback } from "react";
import { Title3, SearchBox, Button, Spinner } from "@fluentui/react-components";
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
    <div className="flex flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
        <Title3>历史记录</Title3>
        {records.length > 0 && (
          <Button
            appearance="subtle"
            icon={<DeleteRegular />}
            onClick={clearAll}
          >
            清空全部
          </Button>
        )}
      </div>

      <SearchBox
        placeholder="搜索历史记录…"
        value={searchQuery}
        onChange={(_, data) => setSearchQuery(data.value)}
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
