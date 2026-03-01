/**
 * 设置配置生命周期 hook。
 *
 * 集中管理 AppConfig 的加载、更新、保存，避免各 tab 散落 invoke 调用。
 * Dialog 关闭时自动保存。
 */
import { useState, useEffect, useCallback, useRef } from "react";
import type { AppConfig } from "../../../shared/lib/types";
import { useAppStore } from "../../../shared/stores/appStore";
import { useUIStore } from "../../../shared/stores/uiStore";
import { createLogger } from "../../../shared/lib/logger";

const log = createLogger("useConfig");

export interface UseConfigReturn {
  config: AppConfig | null;
  isLoading: boolean;
  saveStatus: string;
  updateConfig: (updater: (prev: AppConfig) => AppConfig) => void;
  saveConfig: () => Promise<void>;
}

export function useConfig(): UseConfigReturn {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [saveStatus, setSaveStatus] = useState("");
  const setAppConfig = useAppStore((s) => s.setConfig);
  const settingsOpen = useUIStore((s) => s.settingsOpen);
  const configRef = useRef(config);
  configRef.current = config;

  // 挂载时加载配置
  useEffect(() => {
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const cfg = await invoke<AppConfig>("get_config");
        setConfig(cfg);
        setAppConfig(cfg);
      } catch (e) {
        log.error("[useConfig] 加载配置失败:", e);
        useUIStore.getState().showToast({ type: "error", title: "加载配置失败" });
      }
      setIsLoading(false);
    })();
  }, [setAppConfig]);

  const updateConfig = useCallback(
    (updater: (prev: AppConfig) => AppConfig) => {
      setConfig((prev) => (prev ? updater(prev) : prev));
    },
    [],
  );

  const saveConfig = useCallback(async () => {
    const cfg = configRef.current;
    if (!cfg) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_config", { config: cfg });
      setAppConfig(cfg);
      setSaveStatus("已保存");
      setTimeout(() => setSaveStatus(""), 2000);
    } catch (e) {
      log.error("[useConfig] 保存配置失败:", e);
      setSaveStatus("保存失败");
      setTimeout(() => setSaveStatus(""), 2000);
    }
  }, [setAppConfig]);

  // Dialog 关闭时自动保存
  const prevOpen = useRef(settingsOpen);
  useEffect(() => {
    if (prevOpen.current && !settingsOpen) {
      saveConfig();
    }
    prevOpen.current = settingsOpen;
  }, [settingsOpen, saveConfig]);

  return { config, isLoading, saveStatus, updateConfig, saveConfig };
}
