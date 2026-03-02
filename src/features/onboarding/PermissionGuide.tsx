/**
 * 权限引导组件 — 全平台，解析 JSON PermissionReport。
 *
 * 按 denied 权限动态渲染卡片：
 * - microphone: 全平台
 * - accessibility / input_monitoring: macOS 专用
 *
 * 窗口获焦时自动重检；all_granted 时自动完成。
 */
import { useState, useEffect, useCallback } from "react";
import { Card, Title3, Text, Button, Spinner, Badge } from "@fluentui/react-components";
import {
  ShieldCheckmarkRegular,
  SettingsRegular,
  ArrowSyncRegular,
  MicRegular,
} from "@fluentui/react-icons";
import type { PermissionReport } from "../../shared/lib/types";

interface PermissionGuideProps {
  onComplete: () => void;
}

type CheckState = "checking" | "needs_permissions" | "all_granted";

/** 权限项配置 */
const PERMISSION_ITEMS: Array<{
  key: keyof Omit<PermissionReport, "all_granted">;
  title: string;
  description: string;
  target: string;
  icon: typeof MicRegular;
}> = [
  {
    key: "microphone",
    title: "麦克风权限",
    description: "听语轩需要麦克风权限来录制语音。",
    target: "microphone",
    icon: MicRegular,
  },
  {
    key: "accessibility",
    title: "辅助功能权限",
    description: "听语轩需要辅助功能权限来将文本注入到其他应用。",
    target: "accessibility",
    icon: ShieldCheckmarkRegular,
  },
  {
    key: "input_monitoring",
    title: "输入监控权限",
    description: "听语轩需要输入监控权限来响应全局快捷键。",
    target: "input_monitoring",
    icon: ShieldCheckmarkRegular,
  },
];

export default function PermissionGuide({ onComplete }: PermissionGuideProps) {
  const [state, setState] = useState<CheckState>("checking");
  const [report, setReport] = useState<PermissionReport | null>(null);

  const checkPermissions = useCallback(async () => {
    setState("checking");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const json = await invoke<string>("check_platform_permissions");
      const parsed = JSON.parse(json) as PermissionReport;
      setReport(parsed);
      if (parsed.all_granted) {
        setState("all_granted");
        onComplete();
      } else {
        setState("needs_permissions");
      }
    } catch {
      // 非 Tauri 环境或命令不存在，视为已授权
      onComplete();
    }
  }, [onComplete]);

  // 初次检查
  useEffect(() => {
    checkPermissions();
  }, [checkPermissions]);

  // 窗口获焦时自动重检
  useEffect(() => {
    let cleanup: (() => void) | undefined;

    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        win.onFocusChanged(({ payload: focused }) => {
          if (focused) checkPermissions();
        }).then((unlisten) => {
          cleanup = unlisten;
        });
      })
      .catch(() => {});

    return () => { cleanup?.(); };
  }, [checkPermissions]);

  if (state === "checking") {
    return (
      <Card className="flex flex-col items-center gap-4 p-8 max-w-md mx-auto">
        <Spinner size="medium" label="检查系统权限…" />
      </Card>
    );
  }

  if (state === "all_granted" || !report) return null;

  // 筛选出需要授权的权限项
  const deniedItems = PERMISSION_ITEMS.filter(
    (item) => report[item.key] === "denied",
  );

  const openSettings = async (target: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_permission_settings", { target });
    } catch {
      // ignore
    }
  };

  return (
    <Card className="flex flex-col gap-6 p-8 max-w-md mx-auto">
      <div className="flex items-center gap-3">
        <ShieldCheckmarkRegular className="text-3xl text-blue-600" />
        <Title3>需要系统权限</Title3>
      </div>

      <Text className="text-gray-600">
        听语轩需要以下权限才能正常工作，请逐项授权：
      </Text>

      {/* 权限卡片列表 */}
      <div className="flex flex-col gap-3">
        {deniedItems.map((item) => (
          <div
            key={item.key}
            className="flex items-center justify-between p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/50"
          >
            <div className="flex items-center gap-2">
              <item.icon className="text-red-500" />
              <div>
                <p className="text-sm font-medium">{item.title}</p>
                <p className="text-xs text-gray-500 dark:text-gray-400">
                  {item.description}
                </p>
              </div>
            </div>
            <Button
              appearance="primary"
              size="small"
              icon={<SettingsRegular />}
              onClick={() => openSettings(item.target)}
            >
              打开设置
            </Button>
          </div>
        ))}
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
