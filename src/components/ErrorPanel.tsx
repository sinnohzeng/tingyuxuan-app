import type { UserAction } from "../lib/types";

interface ErrorPanelProps {
  message: string;
  action: UserAction;
  rawTranscript: string | null;
  onRetry: () => void;
  onInsertRaw: () => void;
  onDismiss: () => void;
  onOpenSettings: () => void;
}

export default function ErrorPanel({
  message,
  action,
  rawTranscript,
  onRetry,
  onInsertRaw,
  onDismiss,
  onOpenSettings,
}: ErrorPanelProps) {
  return (
    <div className="flex flex-col items-center gap-2 px-4 py-2 w-full">
      <p className="text-red-400 text-xs truncate max-w-[300px]">{message}</p>
      <div className="flex gap-2">
        {action === "RetryOrQueue" && (
          <>
            <ActionButton label="重试" onClick={onRetry} primary />
            <ActionButton label="稍后处理" onClick={onDismiss} />
          </>
        )}
        {action === "InsertRawOrRetry" && (
          <>
            {rawTranscript && (
              <ActionButton label="插入原始转写" onClick={onInsertRaw} primary />
            )}
            <ActionButton label="重试润色" onClick={onRetry} />
            <ActionButton label="关闭" onClick={onDismiss} />
          </>
        )}
        {action === "CheckApiKey" && (
          <>
            <ActionButton label="前往设置" onClick={onOpenSettings} primary />
            <ActionButton label="关闭" onClick={onDismiss} />
          </>
        )}
        {action === "WaitAndRetry" && (
          <>
            <ActionButton label="重试" onClick={onRetry} primary />
            <ActionButton label="关闭" onClick={onDismiss} />
          </>
        )}
        {action === "CheckMicrophone" && (
          <>
            <ActionButton label="知道了" onClick={onDismiss} primary />
          </>
        )}
      </div>
    </div>
  );
}

function ActionButton({
  label,
  onClick,
  primary = false,
}: {
  label: string;
  onClick: () => void;
  primary?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`px-3 py-1 rounded-lg text-xs font-medium transition-colors ${
        primary
          ? "bg-blue-500 text-white hover:bg-blue-600"
          : "bg-gray-700 text-gray-300 hover:bg-gray-600"
      }`}
    >
      {label}
    </button>
  );
}
