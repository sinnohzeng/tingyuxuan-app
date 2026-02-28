import { useState, useCallback, useEffect, useRef } from "react";

interface PermissionGuideProps {
  onComplete: () => void;
}

type PermissionStatus =
  | "checking"
  | "granted"
  | "accessibility_required"
  | "input_monitoring_required"
  | "both_required";

/** 权限轮询间隔（毫秒） */
const PERMISSION_POLL_INTERVAL = 2000;

/**
 * macOS 权限引导组件。
 *
 * 检测辅助功能 + 输入监控权限状态，引导用户逐项授权。
 * 支持自动轮询 — 用户在系统设置中授权后即时进入下一步。
 * 非 macOS 平台后端始终返回 "granted"，此组件不会显示。
 */
export default function PermissionGuide({ onComplete }: PermissionGuideProps) {
  const [status, setStatus] = useState<PermissionStatus>("checking");
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const checkPermissions = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<string>("check_platform_permissions");
      setStatus(result as PermissionStatus);
    } catch {
      // Tauri 不可用（开发模式），视为已授权
      setStatus("granted");
    }
  }, []);

  // 初始检测
  useEffect(() => {
    checkPermissions();
  }, [checkPermissions]);

  // 自动轮询：权限未授时每 2 秒检测，授权后自动停止
  useEffect(() => {
    if (status === "granted" || status === "checking") {
      // 已授权或正在首次检测 — 不轮询
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
      return;
    }

    // 启动轮询
    timerRef.current = setInterval(checkPermissions, PERMISSION_POLL_INTERVAL);

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [status, checkPermissions]);

  // 已授权 → 自动完成
  useEffect(() => {
    if (status === "granted") onComplete();
  }, [status, onComplete]);

  const handleOpenSettings = useCallback(async (target?: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_permission_settings", { target: target ?? null });
    } catch {
      // Dev mode fallback
    }
  }, []);

  // 检测中或已授权时不渲染
  if (status === "checking" || status === "granted") return null;

  const accessibilityGranted =
    status === "input_monitoring_required";
  const inputMonitoringGranted =
    status === "accessibility_required";

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-50 p-8">
      <div className="w-full max-w-md space-y-6">
        <div className="text-center">
          <div className="w-14 h-14 mx-auto mb-4 bg-amber-100 rounded-2xl flex items-center justify-center">
            <svg
              className="w-7 h-7 text-amber-600"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth={1.5}
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M12 9v3.75m0-10.036A11.959 11.959 0 0 1 3.598 6 11.99 11.99 0 0 0 3 9.75c0 5.592 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.25-8.25-3.286Zm0 13.036h.008v.008H12v-.008Z"
              />
            </svg>
          </div>
          <h2 className="text-xl font-semibold text-gray-800">
            需要系统权限
          </h2>
          <p className="text-sm text-gray-500 mt-2">
            听语轩需要以下权限才能正常工作
          </p>
        </div>

        <div className="space-y-3">
          {/* 辅助功能 */}
          <div className={`px-4 py-3 rounded-lg border ${
            accessibilityGranted
              ? "bg-green-50 border-green-200"
              : "bg-white border-gray-200"
          }`}>
            <div className="flex items-center justify-between">
              <div>
                <div className="flex items-center gap-2">
                  <div className={`w-2 h-2 rounded-full ${
                    accessibilityGranted ? "bg-green-500" : "bg-amber-500"
                  }`} />
                  <span className="text-sm font-medium text-gray-800">
                    辅助功能
                  </span>
                </div>
                <p className="text-xs text-gray-500 mt-1 ml-4">
                  用于在当前应用中插入语音转写的文本
                </p>
              </div>
              {accessibilityGranted ? (
                <span className="text-xs text-green-600 font-medium">
                  已授权
                </span>
              ) : (
                <button
                  onClick={() => handleOpenSettings("accessibility")}
                  className="text-xs text-blue-500 hover:text-blue-600 font-medium"
                >
                  去设置
                </button>
              )}
            </div>
          </div>

          {/* 输入监控 */}
          <div className={`px-4 py-3 rounded-lg border ${
            inputMonitoringGranted
              ? "bg-green-50 border-green-200"
              : "bg-white border-gray-200"
          }`}>
            <div className="flex items-center justify-between">
              <div>
                <div className="flex items-center gap-2">
                  <div className={`w-2 h-2 rounded-full ${
                    inputMonitoringGranted ? "bg-green-500" : "bg-amber-500"
                  }`} />
                  <span className="text-sm font-medium text-gray-800">
                    输入监控
                  </span>
                </div>
                <p className="text-xs text-gray-500 mt-1 ml-4">
                  用于监听 Fn 快捷键以启动/停止语音输入
                </p>
              </div>
              {inputMonitoringGranted ? (
                <span className="text-xs text-green-600 font-medium">
                  已授权
                </span>
              ) : (
                <button
                  onClick={() => handleOpenSettings("input_monitoring")}
                  className="text-xs text-blue-500 hover:text-blue-600 font-medium"
                >
                  去设置
                </button>
              )}
            </div>
          </div>
        </div>

        <div className="px-4 py-3 bg-blue-50 rounded-lg border border-blue-100">
          <p className="text-xs text-blue-700 leading-relaxed">
            请在弹出的系统设置中，找到"听语轩"并勾选启用。
            授权后页面会自动检测并更新状态。
          </p>
        </div>

        <div className="flex flex-col gap-3">
          <button
            onClick={() => handleOpenSettings()}
            className="w-full py-2.5 bg-blue-500 text-white text-sm rounded-lg
                       hover:bg-blue-600 transition-colors"
          >
            打开系统设置
          </button>
          <button
            onClick={checkPermissions}
            className="w-full py-2.5 bg-white text-gray-700 text-sm rounded-lg
                       border border-gray-300 hover:bg-gray-50 transition-colors"
          >
            手动检测
          </button>
        </div>

        <div className="text-center">
          <button
            onClick={onComplete}
            className="text-xs text-gray-400 hover:text-gray-600 transition-colors"
          >
            稍后设置，先跳过
          </button>
        </div>
      </div>
    </div>
  );
}
