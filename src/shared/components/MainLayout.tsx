/**
 * 主窗口 Shell — 左侧导航 + 右侧内容区。
 *
 * - FluentProvider 包装整个布局，自动跟随系统亮/暗主题。
 * - 左侧：品牌导航栏（Logo + 页面导航 + 设置齿轮）。
 * - 右侧：react-router <Outlet />。
 */
import { lazy, Suspense, useCallback, useEffect } from "react";
import { Outlet, NavLink, useNavigate } from "react-router-dom";
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
import { useTauriEvent } from "../hooks/useTauriEvent";
import { useUIStore } from "../stores/uiStore";
import ErrorBoundary from "./ErrorBoundary";
import logoSrc from "../../assets/logo.svg";

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
  const navigate = useNavigate();

  // 首次启动检测 — 未完成引导则重定向
  useEffect(() => {
    if (!localStorage.getItem("onboarding_complete")) {
      navigate("/onboarding", { replace: true });
    }
  }, [navigate]);

  // 托盘菜单事件联动
  useTauriEvent("open-settings", openSettings);
  useTauriEvent(
    "open-history",
    useCallback(() => navigate("/main/history"), [navigate]),
  );

  return (
    <FluentProvider theme={theme} className="flex h-screen bg-gray-50 dark:bg-gray-950">
      {/* 侧边栏 */}
      <nav
        className="flex flex-col w-16 border-r border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shrink-0"
        aria-label="主导航"
      >
        {/* Logo */}
        <div className="flex items-center justify-center pt-4 pb-3">
          <img
            src={logoSrc}
            alt="听语轩"
            className="w-8 h-8 rounded-lg object-cover"
          />
        </div>

        {/* 导航项 */}
        <div className="flex flex-col items-center gap-1 flex-1 pt-2">
          {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              end={to === "/main"}
              aria-label={label}
              title={label}
              className={({ isActive }) =>
                `relative flex items-center justify-center w-11 h-11 rounded-xl transition-all duration-150 ${
                  isActive
                    ? "bg-blue-50 dark:bg-blue-950 text-blue-600 dark:text-blue-400 shadow-sm"
                    : "text-gray-400 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-600 dark:hover:text-gray-300"
                }`
              }
            >
              {({ isActive }) => (
                <>
                  {isActive && (
                    <span className="absolute left-0 top-2 bottom-2 w-[3px] rounded-r-full bg-blue-600 dark:bg-blue-400" />
                  )}
                  <Icon className="text-xl" />
                </>
              )}
            </NavLink>
          ))}
        </div>

        {/* 底部齿轮 */}
        <div className="flex items-center justify-center pb-4">
          <button
            onClick={() => openSettings()}
            aria-label="设置"
            title="设置"
            className="flex items-center justify-center w-11 h-11 rounded-xl text-gray-400 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-600 dark:hover:text-gray-300 transition-all duration-150"
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
