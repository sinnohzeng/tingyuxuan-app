/**
 * 可复用的连接测试按钮。
 *
 * 内部使用 useConnectionTest hook，对外仅需指定命令名。
 */
import { Button, Spinner, Badge } from "@fluentui/react-components";
import { useConnectionTest } from "../hooks/useConnectionTest";

interface ConnectionTestButtonProps {
  command: string;
  label: string;
}

const STATUS_MAP = {
  success: { color: "success" as const, text: "连接成功" },
  failed: { color: "danger" as const, text: "连接失败" },
};

export default function ConnectionTestButton({ command, label }: ConnectionTestButtonProps) {
  const { status, runTest } = useConnectionTest(command);

  return (
    <div className="flex items-center gap-3">
      <Button
        appearance="secondary"
        disabled={status === "testing"}
        onClick={runTest}
        icon={status === "testing" ? <Spinner size="tiny" /> : undefined}
      >
        {label}
      </Button>
      {(status === "success" || status === "failed") && (
        <Badge appearance="filled" color={STATUS_MAP[status].color}>
          {STATUS_MAP[status].text}
        </Badge>
      )}
    </div>
  );
}
