/**
 * 前端埋点 — 通过 Rust 后端统一上报到 SLS。
 *
 * 前端不直接调 SLS API，统一经 Tauri command 汇总。
 * 非 Tauri 环境或未初始化时静默忽略。
 */
export async function trackEvent(
  event: string,
  props?: Record<string, unknown>,
): Promise<void> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("report_telemetry_event", {
      event: JSON.stringify({ event_type: event, ...props }),
    });
  } catch {
    // 非 Tauri 环境或未初始化，静默忽略
  }
}
