/**
 * 单个 API Key 的 CRUD + 状态管理 hook。
 *
 * 封装 key 的加载、保存、显示/隐藏、状态反馈，消除 STT/LLM 之间的重复代码。
 */
import { useState, useEffect, useCallback, useRef } from "react";

export interface UseApiKeyReturn {
  keyValue: string;
  showKey: boolean;
  keyStatus: string;
  setKeyValue: (v: string) => void;
  toggleShowKey: () => void;
  saveKey: () => Promise<void>;
}

export function useApiKey(service: "llm"): UseApiKeyReturn {
  const [keyValue, setKeyValue] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [keyStatus, setKeyStatus] = useState("");
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  // 挂载时检查 key 是否已配置
  useEffect(() => {
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const existing = await invoke<string | null>("get_api_key", { service });
        setKeyStatus(existing ? "已配置" : "未配置");
      } catch (e) {
        console.error(`[useApiKey] 加载 ${service} API Key 状态失败:`, e);
      }
    })();
  }, [service]);

  const saveKey = useCallback(async () => {
    if (!keyValue.trim()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_api_key", { service, key: keyValue.trim() });
      setKeyStatus("已配置");
      setKeyValue("");
    } catch (e) {
      console.error(`[useApiKey] 保存 ${service} API Key 失败:`, e);
      setKeyStatus("保存失败");
    }
    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(
      () => setKeyStatus((s) => (s === "保存失败" ? "" : s)),
      3000,
    );
  }, [keyValue, service]);

  const toggleShowKey = useCallback(() => setShowKey((v) => !v), []);

  return { keyValue, showKey, keyStatus, setKeyValue, toggleShowKey, saveKey };
}
