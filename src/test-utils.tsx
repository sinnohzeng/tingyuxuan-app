/**
 * 测试工具 — 提供 FluentProvider + MemoryRouter 包装器。
 *
 * 依赖 Fluent 2 主题 token 的组件（HomePage、SettingsDialog 等）
 * 必须使用 renderWithProviders。
 * 旧测试（FloatingBar、ErrorPanel 等）不强制迁移。
 */
import { render, type RenderOptions } from "@testing-library/react";
import { FluentProvider, webLightTheme } from "@fluentui/react-components";
import { MemoryRouter, type MemoryRouterProps } from "react-router-dom";
import { useAppStore } from "./shared/stores/appStore";
import { useUIStore } from "./shared/stores/uiStore";
import { useStatsStore } from "./shared/stores/statsStore";

interface TestOptions extends Omit<RenderOptions, "wrapper"> {
  initialEntries?: MemoryRouterProps["initialEntries"];
}

function createWrapper(initialEntries?: MemoryRouterProps["initialEntries"]) {
  return function TestProviders({ children }: { children: React.ReactNode }) {
    return (
      <FluentProvider theme={webLightTheme}>
        <MemoryRouter initialEntries={initialEntries}>{children}</MemoryRouter>
      </FluentProvider>
    );
  };
}

export function renderWithProviders(ui: React.ReactElement, options?: TestOptions) {
  const { initialEntries, ...renderOptions } = options ?? {};
  return render(ui, { wrapper: createWrapper(initialEntries), ...renderOptions });
}

/** 在 beforeEach 中调用，重置所有 Zustand store */
export function resetStores() {
  useAppStore.getState().reset();
  useUIStore.setState({ settingsOpen: false, settingsTab: "settings", toasts: [] });
  useStatsStore.setState({ stats: null, lastFetched: null, isLoading: false, error: null });
}
