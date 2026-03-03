import { useEffect, useState, useCallback, useMemo, useRef } from "react";
import DOMPurify from "dompurify";
import { renderMarkdown } from "../../shared/lib/markdown";
import { trackEvent } from "../../shared/lib/telemetry";

interface ResultPanelProps {
  result: string;
  onCopy: () => void;
  onInsert: () => void;
  onDismiss: () => void;
}

export default function ResultPanel({
  result,
  onCopy,
  onInsert,
  onDismiss,
}: ResultPanelProps) {
  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    };
  }, []);

  // Resize window to show result panel.
  useEffect(() => {
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow, LogicalSize }) => {
        getCurrentWindow()
          .setSize(new LogicalSize(420, 360))
          .catch(() => {});
      })
      .catch(() => {});

    return () => {
      import("@tauri-apps/api/window")
        .then(({ getCurrentWindow, LogicalSize }) => {
          getCurrentWindow()
            .setSize(new LogicalSize(220, 56))
            .catch(() => {});
        })
        .catch(() => {});
    };
  }, []);

  const handleCopy = useCallback(() => {
    trackEvent("user_action", { action: "result_copy" });
    onCopy();
    setCopied(true);
    if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    copyTimerRef.current = setTimeout(() => setCopied(false), 1000);
  }, [onCopy]);

  const renderedHtml = useMemo(() => DOMPurify.sanitize(renderMarkdown(result)), [result]);

  return (
    <div className="flex flex-col w-full h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-gray-700/50">
        <h2 className="text-sm font-medium text-gray-300">AI 助手</h2>
        <button
          onClick={onDismiss}
          className="text-gray-400 hover:text-gray-200 transition-colors"
          title="关闭"
          aria-label="关闭结果面板"
        >
          <svg
            className="w-4 h-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      {/* Content */}
      <div
        className="flex-1 overflow-y-auto px-4 py-3 text-sm text-gray-200 leading-relaxed
                    prose prose-invert prose-sm max-w-none
                    [&_code]:bg-gray-700/50 [&_code]:px-1 [&_code]:rounded
                    [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:pl-5
                    [&_li]:my-0.5 [&_p]:my-1 [&_h3]:text-base [&_h3]:font-medium [&_h3]:mt-2"
        style={{ maxHeight: "248px" }}
        dangerouslySetInnerHTML={{ __html: renderedHtml }}
      />

      {/* Actions */}
      <div className="flex items-center justify-center gap-3 px-4 py-2 border-t border-gray-700/50">
        <button
          onClick={handleCopy}
          className="px-3 py-1.5 text-xs text-gray-300 hover:text-white
                     bg-gray-700/50 hover:bg-gray-700 rounded-lg transition-colors"
        >
          {copied ? "已复制" : "复制"}
        </button>
        <button
          onClick={onInsert}
          className="px-3 py-1.5 text-xs text-white
                     bg-blue-600 hover:bg-blue-500 rounded-lg transition-colors"
        >
          插入到光标
        </button>
      </div>
    </div>
  );
}
