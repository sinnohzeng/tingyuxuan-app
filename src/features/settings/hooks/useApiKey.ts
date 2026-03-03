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

type KeyStatus = UseApiKeyReturn["keyStatus"];

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

  useEffect(() => {
    void loadKeyStatus(service, setKeyStatus, setMaskedKey);
  }, [service]);

  const saveKey = useSaveKey(
    service,
    keyValue,
    timerRef,
    setMaskedKey,
    setKeyStatus,
    setKeyValue,
    setIsEditing,
  );
  const { startEditing, cancelEditing } = useEditHandlers(setIsEditing, setKeyValue);

  return { keyValue, maskedKey, keyStatus, isEditing, setKeyValue, startEditing, cancelEditing, saveKey };
}

async function loadKeyStatus(
  service: "llm",
  setKeyStatus: (status: KeyStatus) => void,
  setMaskedKey: (masked: string) => void,
) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const existing = await invoke<string | null>("get_api_key", { service });
    if (!existing) {
      setKeyStatus("unconfigured");
      return;
    }
    setKeyStatus("configured");
    setMaskedKey(maskApiKey(existing));
  } catch (e) {
    log.error(`加载 ${service} API Key 状态失败:`, e);
    setKeyStatus("unconfigured");
  }
}

function handleSaveFailed(
  service: "llm",
  error: unknown,
  timerRef: { current: ReturnType<typeof setTimeout> | undefined },
  setKeyStatus: (status: KeyStatus) => void,
) {
  log.error(`保存 ${service} API Key 失败:`, error);
  setKeyStatus("save_failed");
  clearTimeout(timerRef.current);
  timerRef.current = setTimeout(() => setKeyStatus("configured"), 3000);
}

function useSaveKey(
  service: "llm",
  keyValue: string,
  timerRef: { current: ReturnType<typeof setTimeout> | undefined },
  setMaskedKey: (masked: string) => void,
  setKeyStatus: (status: KeyStatus) => void,
  setKeyValue: (value: string) => void,
  setIsEditing: (editing: boolean) => void,
) {
  return useCallback(async () => {
    const trimmed = keyValue.trim();
    if (!trimmed) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_api_key", { service, key: trimmed });
      setMaskedKey(maskApiKey(trimmed));
      setKeyStatus("configured");
      setKeyValue("");
      setIsEditing(false);
    } catch (e) {
      handleSaveFailed(service, e, timerRef, setKeyStatus);
    }
  }, [keyValue, service, setIsEditing, setKeyStatus, setKeyValue, setMaskedKey, timerRef]);
}

function useEditHandlers(
  setIsEditing: (editing: boolean) => void,
  setKeyValue: (value: string) => void,
) {
  const resetInput = useCallback(() => setKeyValue(""), [setKeyValue]);
  const startEditing = useCallback(() => {
    setIsEditing(true);
    resetInput();
  }, [setIsEditing, resetInput]);
  const cancelEditing = useCallback(() => {
    setIsEditing(false);
    resetInput();
  }, [setIsEditing, resetInput]);
  return { startEditing, cancelEditing };
}
