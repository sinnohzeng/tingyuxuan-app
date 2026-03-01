type Level = "debug" | "info" | "warn" | "error";

const LEVEL_VALUE: Record<Level, number> = { debug: 0, info: 1, warn: 2, error: 3 };
// Production: info+（不是 warn+）— 用户报 bug 时需要生命周期日志
const isDev: boolean = (import.meta as unknown as { env?: { DEV?: boolean } }).env?.DEV ?? false;
const threshold: number = LEVEL_VALUE[isDev ? "debug" : "info"];

let _sessionId: string | null = null;
export function setLogSession(id: string | null): void { _sessionId = id; }

function emit(level: Level, tag: string, msg: string, data?: unknown): void {
  if (LEVEL_VALUE[level] < threshold) return;
  const prefix = _sessionId
    ? `[TYX:${tag}:${_sessionId.slice(0, 8)}]`
    : `[TYX:${tag}]`;
  const fn = console[level];
  data !== undefined ? fn(prefix, msg, data) : fn(prefix, msg);
}

export interface Logger {
  debug(msg: string, data?: unknown): void;
  info(msg: string, data?: unknown): void;
  warn(msg: string, data?: unknown): void;
  error(msg: string, data?: unknown): void;
}

export function createLogger(tag: string): Logger {
  return {
    debug: (msg, data?) => emit("debug", tag, msg, data),
    info:  (msg, data?) => emit("info",  tag, msg, data),
    warn:  (msg, data?) => emit("warn",  tag, msg, data),
    error: (msg, data?) => emit("error", tag, msg, data),
  };
}
