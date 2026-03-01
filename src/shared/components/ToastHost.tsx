/**
 * Toast 通知宿主 — 监听 uiStore.toasts 变化，通过 Fluent Toaster 渲染通知。
 *
 * 挂载在 MainLayout 的 FluentProvider 内部。
 */
import { useEffect, useRef } from "react";
import {
  Toaster,
  useToastController,
  useId,
  Toast,
  ToastTitle,
  ToastBody,
  type ToastIntent,
} from "@fluentui/react-components";
import { useUIStore, type ToastMessage } from "../stores/uiStore";

const TIMEOUT_MS: Record<ToastMessage["type"], number> = {
  success: 4000,
  info: 5000,
  warning: 6000,
  error: 8000,
};

export default function ToastHost() {
  const toasterId = useId("app-toaster");
  const { dispatchToast } = useToastController(toasterId);
  const toasts = useUIStore((s) => s.toasts);
  const dismissToast = useUIStore((s) => s.dismissToast);
  const dispatched = useRef(new Set<string>());

  // 通过 ref 稳定化回调引用，避免 useEffect 因 dispatchToast/dismissToast
  // 每次渲染返回新引用而无限触发。
  const dispatchRef = useRef(dispatchToast);
  dispatchRef.current = dispatchToast;
  const dismissRef = useRef(dismissToast);
  dismissRef.current = dismissToast;

  useEffect(() => {
    for (const msg of toasts) {
      if (dispatched.current.has(msg.id)) continue;
      dispatched.current.add(msg.id);

      dispatchRef.current(
        <Toast>
          <ToastTitle>{msg.title}</ToastTitle>
          {msg.body && <ToastBody>{msg.body}</ToastBody>}
        </Toast>,
        {
          intent: msg.type as ToastIntent,
          timeout: TIMEOUT_MS[msg.type],
          onStatusChange: (_e, data) => {
            if (data.status === "unmounted") {
              dismissRef.current(msg.id);
              dispatched.current.delete(msg.id);
            }
          },
        },
      );
    }
  }, [toasts]);

  return <Toaster toasterId={toasterId} position="bottom-end" />;
}
