/**
 * 主窗口 Shell — 左侧导航 + 右侧内容区。
 *
 * - FluentProvider 包装整个布局，自动跟随系统亮/暗主题。
 * - 左侧：垂直导航栏（首页 / 历史 / 词典 / 设置齿轮）。
 * - 右侧：react-router <Outlet />。
 * - 底部齿轮触发 SettingsDialog（Sprint 2 接入）。
 */
import { lazy, Suspense } from "react";
import { Outlet, NavLink } from "react-router-dom";
import {
  FluentProvider,
  Spinner,
} from "@fluentui/react-components";

const SettingsDialog = lazy(() => import("../../features/settings/SettingsDialog"));
import ToastHost from "./ToastHost";
import {
  HomeRegular,
  HomeFilled,
  HistoryRegular,
  HistoryFilled,
  BookRegular,
  BookFilled,
  SettingsRegular,
  bundleIcon,
} from "@fluentui/react-icons";
import { useSystemTheme } from "../hooks/useSystemTheme";
import { useUIStore } from "../stores/uiStore";
import ErrorBoundary from "./ErrorBoundary";

const HomeIcon = bundleIcon(HomeFilled, HomeRegular);
const HistoryIcon = bundleIcon(HistoryFilled, HistoryRegular);
const DictionaryIcon = bundleIcon(BookFilled, BookRegular);

interface NavItem {
  to: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/main", label: "首页", icon: HomeIcon },
  { to: "/main/history", label: "历史", icon: HistoryIcon },
  { to: "/main/dictionary", label: "词典", icon: DictionaryIcon },
];

function LoadingFallback() {
  return (
    <div className="flex items-center justify-center h-full">
      <Spinner size="medium" label="加载中…" />
    </div>
  );
}

export default function MainLayout() {
  const theme = useSystemTheme();
  const openSettings = useUIStore((s) => s.openSettings);

  return (
    <FluentProvider theme={theme} className="flex h-screen">
      {/* 侧边栏 */}
      <nav
        className="flex flex-col w-14 border-r shrink-0"
        aria-label="主导航"
      >
        <div className="flex flex-col items-center gap-1 pt-4 flex-1">
          {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              end={to === "/main"}
              aria-label={label}
              title={label}
              className={({ isActive }) =>
                `flex items-center justify-center w-10 h-10 rounded-lg transition-colors ${
                  isActive
                    ? "bg-blue-100 text-blue-700"
                    : "text-gray-500 hover:bg-gray-100 hover:text-gray-700"
                }`
              }
            >
              <Icon className="text-xl" />
            </NavLink>
          ))}
        </div>

        {/* 底部齿轮 */}
        <div className="flex items-center justify-center pb-4">
          <button
            onClick={() => openSettings()}
            aria-label="设置"
            title="设置"
            className="flex items-center justify-center w-10 h-10 rounded-lg text-gray-500 hover:bg-gray-100 hover:text-gray-700 transition-colors"
          >
            <SettingsRegular className="text-xl" />
          </button>
        </div>
      </nav>

      {/* 内容区 */}
      <main className="flex-1 overflow-y-auto">
        <ErrorBoundary>
          <Suspense fallback={<LoadingFallback />}>
            <Outlet />
          </Suspense>
        </ErrorBoundary>
      </main>

      <Suspense>
        <SettingsDialog />
      </Suspense>
      <ToastHost />
    </FluentProvider>
  );
}
