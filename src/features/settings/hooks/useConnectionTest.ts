/**
 * 连接测试 hook。
 *
 * 参数化 Tauri 命令名，复用于 STT 和 LLM 测试。
 */
import { useState, useCallback, useRef, useEffect } from "react";
import { createLogger } from "../../../shared/lib/logger";

const log = createLogger("useConnectionTest");

export type TestStatus = "idle" | "testing" | "success" | "failed";

export interface UseConnectionTestReturn {
  status: TestStatus;
  runTest: () => Promise<void>;
}

export function useConnectionTest(command: string): UseConnectionTestReturn {
  const [status, setStatus] = useState<TestStatus>("idle");
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  const runTest = useCallback(async () => {
    setStatus("testing");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const ok = await invoke<boolean>(command);
      setStatus(ok ? "success" : "failed");
    } catch (e) {
      log.error(`${command} 测试失败:`, e);
      setStatus("failed");
    }
    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => setStatus("idle"), 3000);
  }, [command]);

  return { status, runTest };
}
