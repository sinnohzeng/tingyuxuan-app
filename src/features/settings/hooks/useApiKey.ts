/**
 * 单个 API Key 的 CRUD + 状态管理 hook。
 *
 * 已配置状态显示掩码（sk-****xxxx），不允许查看完整 key。
 */
import { useState, useEffect, useCallback, useRef } from "react";
import { createLogger } from "../../../shared/lib/logger";

const log = createLogger("useApiKey");

export interface UseApiKeyReturn {
  keyValue: string;
  maskedKey: string;
  keyStatus: "loading" | "configured" | "unconfigured" | "save_failed";
  isEditing: boolean;
  setKeyValue: (v: string) => void;
  startEditing: () => void;
  cancelEditing: () => void;
  saveKey: () => Promise<void>;
}

/** 生成掩码：sk-****xxxx（末 4 位可见），短 key 全掩码 */
function maskApiKey(raw: string): string {
  if (raw.length <= 8) return "*".repeat(raw.length);
  const prefix = raw.slice(0, 3);
  const suffix = raw.slice(-4);
  return `${prefix}****${suffix}`;
}

export function useApiKey(service: "llm"): UseApiKeyReturn {
  const [keyValue, setKeyValue] = useState("");
  const [maskedKey, setMaskedKey] = useState("");
  const [keyStatus, setKeyStatus] = useState<UseApiKeyReturn["keyStatus"]>("loading");
  const [isEditing, setIsEditing] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  // 挂载时检查 key 是否已配置
  useEffect(() => {
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const existing = await invoke<string | null>("get_api_key", { service });
        if (existing) {
          setKeyStatus("configured");
          setMaskedKey(maskApiKey(existing));
        } else {
          setKeyStatus("unconfigured");
        }
      } catch (e) {
        log.error(`加载 ${service} API Key 状态失败:`, e);
        setKeyStatus("unconfigured");
      }
    })();
  }, [service]);

  const saveKey = useCallback(async () => {
    if (!keyValue.trim()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_api_key", { service, key: keyValue.trim() });
      setMaskedKey(maskApiKey(keyValue.trim()));
      setKeyStatus("configured");
      setKeyValue("");
      setIsEditing(false);
    } catch (e) {
      log.error(`保存 ${service} API Key 失败:`, e);
      setKeyStatus("save_failed");
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setKeyStatus("configured"), 3000);
    }
  }, [keyValue, service]);

  const startEditing = useCallback(() => {
    setIsEditing(true);
    setKeyValue("");
  }, []);

  const cancelEditing = useCallback(() => {
    setIsEditing(false);
    setKeyValue("");
  }, []);

  return { keyValue, maskedKey, keyStatus, isEditing, setKeyValue, startEditing, cancelEditing, saveKey };
}
