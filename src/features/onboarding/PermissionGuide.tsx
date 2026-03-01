/**
 * 权限引导组件 — macOS 专用，检查辅助功能/输入监控权限。
 *
 * Windows/Linux 上 check_platform_permissions 直接返回 "granted"，自动跳过。
 * 不使用 setInterval 盲轮询，而是用户点击"重新检查"按钮触发。
 */
import { useState, useEffect, useCallback } from "react";
import { Card, Title3, Text, Button, Spinner, Badge } from "@fluentui/react-components";
import {
  ShieldCheckmarkRegular,
  SettingsRegular,
  ArrowSyncRegular,
} from "@fluentui/react-icons";

interface PermissionGuideProps {
  onComplete: () => void;
}

type PermissionStatus =
  | "checking"
  | "granted"
  | "accessibility_required"
  | "input_monitoring_required"
  | "both_required";

const PERMISSION_INFO: Record<string, { title: string; description: string; target?: string }> = {
  accessibility_required: {
    title: "需要辅助功能权限",
    description: "听语轩需要辅助功能权限来将文本注入到其他应用。",
  },
  input_monitoring_required: {
    title: "需要输入监控权限",
    description: "听语轩需要输入监控权限来响应全局快捷键。",
    target: "input_monitoring",
  },
  both_required: {
    title: "需要两项系统权限",
    description: "听语轩需要辅助功能和输入监控权限才能正常工作。",
  },
};

export default function PermissionGuide({ onComplete }: PermissionGuideProps) {
  const [status, setStatus] = useState<PermissionStatus>("checking");

  const checkPermissions = useCallback(async () => {
    setStatus("checking");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<string>("check_platform_permissions");
      setStatus(result as PermissionStatus);
      if (result === "granted") onComplete();
    } catch {
      // 非 Tauri 环境或命令不存在，视为已授权
      onComplete();
    }
  }, [onComplete]);

  useEffect(() => {
    checkPermissions();
  }, [checkPermissions]);

  if (status === "checking") {
    return (
      <Card className="flex flex-col items-center gap-4 p-8 max-w-md mx-auto">
        <Spinner size="medium" label="检查系统权限…" />
      </Card>
    );
  }

  if (status === "granted") return null;

  const info = PERMISSION_INFO[status];

  const openSettings = async (target?: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_permission_settings", { target: target ?? null });
    } catch {
      // ignore
    }
  };

  return (
    <Card className="flex flex-col gap-6 p-8 max-w-md mx-auto">
      <div className="flex items-center gap-3">
        <ShieldCheckmarkRegular className="text-3xl text-blue-600" />
        <Title3>{info.title}</Title3>
      </div>

      <Text className="text-gray-600">{info.description}</Text>

      {/* 权限操作按钮 */}
      <div className="flex flex-col gap-3">
        {(status === "accessibility_required" || status === "both_required") && (
          <Button
            appearance="primary"
            icon={<SettingsRegular />}
            onClick={() => openSettings()}
          >
            打开辅助功能设置
          </Button>
        )}
        {(status === "input_monitoring_required" || status === "both_required") && (
          <Button
            appearance="primary"
            icon={<SettingsRegular />}
            onClick={() => openSettings("input_monitoring")}
          >
            打开输入监控设置
          </Button>
        )}
      </div>

      {/* 重新检查 */}
      <div className="flex items-center gap-3">
        <Button
          appearance="secondary"
          icon={<ArrowSyncRegular />}
          onClick={checkPermissions}
        >
          我已授权，重新检查
        </Button>
        <Badge appearance="tint" color="warning">
          等待授权
        </Badge>
      </div>
    </Card>
  );
}
