/**
 * Tauri 事件监听 hook — 封装 mounted 守卫 + 异步 cleanup 模式。
 *
 * 用法：useTauriEvent("open-settings", handler)
 */
import { useEffect } from "react";

export function useTauriEvent(event: string, handler: () => void) {
  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | undefined;

    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      if (!mounted) return;
      unlisten = await listen(event, handler);
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [event, handler]);
}
