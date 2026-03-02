import type { UserAction } from "../../shared/lib/types";
import { trackEvent } from "../../shared/lib/telemetry";

interface ErrorPanelProps {
  message: string;
  action: UserAction;
  onDismiss: () => void;
  onOpenSettings: () => void;
  onOpenMicSettings: () => void;
}

export default function ErrorPanel({
  message,
  action,
  onDismiss,
  onOpenSettings,
  onOpenMicSettings,
}: ErrorPanelProps) {
  return (
    <div role="alert" className="flex flex-col items-center gap-2 px-4 py-2 w-full">
      <p className="text-red-400 text-xs truncate max-w-[300px]">{message}</p>
      <div className="flex gap-2">
        {action === "Retry" && (
          <>
            <ActionButton label="重试" onClick={() => { trackEvent("user_action", { action: "error_retry" }); onDismiss(); }} primary />
            <ActionButton label="稍后处理" onClick={onDismiss} />
          </>
        )}
        {action === "CheckApiKey" && (
          <>
            <ActionButton label="前往设置" onClick={() => { trackEvent("user_action", { action: "error_open_settings" }); onOpenSettings(); }} primary />
            <ActionButton label="关闭" onClick={onDismiss} />
          </>
        )}
        {action === "WaitAndRetry" && (
          <>
            <ActionButton label="重试" onClick={() => { trackEvent("user_action", { action: "error_retry" }); onDismiss(); }} primary />
            <ActionButton label="关闭" onClick={onDismiss} />
          </>
        )}
        {action === "CheckMicrophone" && (
          <>
            <ActionButton label="打开麦克风设置" onClick={() => { trackEvent("user_action", { action: "error_open_mic_settings" }); onOpenMicSettings(); }} primary />
            <ActionButton label="关闭" onClick={onDismiss} />
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
