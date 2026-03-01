/**
 * 最近转录列表 — 首页展示最近 5 条记录。
 */
import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { Card, CardHeader, Text, Button, Badge } from "@fluentui/react-components";
import { ArrowRightRegular } from "@fluentui/react-icons";
import type { TranscriptRecord } from "../../shared/lib/types";
import { MODE_LABELS, STATUS_CONFIG, formatTime } from "../history/HistoryItem.utils";

export default function RecentTranscripts() {
  const [records, setRecords] = useState<TranscriptRecord[]>([]);
  const navigate = useNavigate();

  useEffect(() => {
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const page = await invoke<TranscriptRecord[]>("get_history_page", {
          limit: 5,
          offset: 0,
        });
        setRecords(page);
      } catch (e) {
        console.error("[RecentTranscripts] 加载最近记录失败:", e);
      }
    })();
  }, []);

  if (records.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <Text weight="semibold" size={400}>最近转录</Text>
        <Button
          appearance="subtle"
          size="small"
          icon={<ArrowRightRegular />}
          iconPosition="after"
          onClick={() => navigate("/main/history")}
        >
          查看全部
        </Button>
      </div>

      {records.map((r) => {
        const statusCfg = STATUS_CONFIG[r.status] ?? STATUS_CONFIG["error"];
        return (
          <Card key={r.id} size="small">
            <CardHeader
              header={
                <Text className="line-clamp-1">
                  {r.processed_text || r.raw_text || "（无文本）"}
                </Text>
              }
              description={
                <div className="flex items-center gap-2">
                  <Text size={200}>{formatTime(r.timestamp)}</Text>
                  <Badge appearance="filled" color={statusCfg.color} size="small">
                    {MODE_LABELS[r.mode] ?? r.mode}
                  </Badge>
                </div>
              }
            />
          </Card>
        );
      })}
    </div>
  );
}
