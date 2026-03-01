/**
 * 单条历史记录卡片。
 */
import { Card, CardHeader, Text, Badge, Button, Tooltip } from "@fluentui/react-components";
import {
  CopyRegular,
  DeleteRegular,
} from "@fluentui/react-icons";
import type { TranscriptRecord } from "../../shared/lib/types";
import { MODE_LABELS, STATUS_CONFIG, formatTime } from "./HistoryItem.utils";

interface HistoryItemProps {
  record: TranscriptRecord;
  onDelete: (id: string) => void;
  onCopy: (text: string) => void;
}

export default function HistoryItem({ record, onDelete, onCopy }: HistoryItemProps) {
  const text = record.processed_text || record.raw_text || "";
  const statusCfg = STATUS_CONFIG[record.status] ?? STATUS_CONFIG["error"];

  return (
    <Card>
      <CardHeader
        header={
          <div className="flex items-center gap-2">
            <Badge appearance="filled" color={statusCfg.color} size="small">
              {MODE_LABELS[record.mode] ?? record.mode}
            </Badge>
            <Badge appearance="outline" color={statusCfg.color} size="small">
              {statusCfg.text}
            </Badge>
            <Text size={200} className="ml-auto">{formatTime(record.timestamp)}</Text>
          </div>
        }
        description={
          <Text className="line-clamp-2 mt-1">{text || "（无文本）"}</Text>
        }
        action={
          <div className="flex gap-1">
            {text && (
              <Tooltip content="复制" relationship="label">
                <Button
                  appearance="subtle"
                  size="small"
                  icon={<CopyRegular />}
                  onClick={() => onCopy(text)}
                  aria-label="复制"
                />
              </Tooltip>
            )}
            <Tooltip content="删除" relationship="label">
              <Button
                appearance="subtle"
                size="small"
                icon={<DeleteRegular />}
                onClick={() => onDelete(record.id)}
                aria-label="删除"
              />
            </Tooltip>
          </div>
        }
      />
    </Card>
  );
}
