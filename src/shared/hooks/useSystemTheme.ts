import { useState, useEffect } from "react";
import type { Theme } from "@fluentui/react-components";
import { lightTheme, darkTheme } from "../lib/theme";

const DARK_QUERY = "(prefers-color-scheme: dark)";

/**
 * 跟随系统亮/暗主题，返回对应的 Fluent 2 Theme 对象。
 */
export function useSystemTheme(): Theme {
  const [isDark, setIsDark] = useState(
    () => window.matchMedia(DARK_QUERY).matches,
  );

  useEffect(() => {
    const mql = window.matchMedia(DARK_QUERY);
    const handler = (e: MediaQueryListEvent) => setIsDark(e.matches);
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, []);

  return isDark ? darkTheme : lightTheme;
}
