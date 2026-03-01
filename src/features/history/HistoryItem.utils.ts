/**
 * 历史记录条目的常量和工具函数。
 */
import type { BadgeProps } from "@fluentui/react-components";

export const MODE_LABELS: Record<string, string> = {
  dictate: "听写",
  translate: "翻译",
  ai_assistant: "AI 助手",
  edit: "语音编辑",
};

export const STATUS_CONFIG: Record<
  string,
  { text: string; color: BadgeProps["color"] }
> = {
  success: { text: "成功", color: "success" },
  failed: { text: "失败", color: "danger" },
  recording: { text: "录音中", color: "informative" },
  processing: { text: "处理中", color: "informative" },
  cancelled: { text: "已取消", color: "subtle" },
  error: { text: "错误", color: "danger" },
};

export function formatTime(timestamp: string): string {
  try {
    const d = new Date(timestamp);
    const now = new Date();
    const isToday = d.toDateString() === now.toDateString();
    const time = d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
    if (isToday) return time;
    return `${d.toLocaleDateString("zh-CN", { month: "short", day: "numeric" })} ${time}`;
  } catch {
    return timestamp;
  }
}
