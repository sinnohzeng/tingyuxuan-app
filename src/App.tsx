import { lazy, Suspense } from "react";
import { Routes, Route } from "react-router-dom";
import ErrorBoundary from "./shared/components/ErrorBoundary";
import FloatingBar from "./features/recording/FloatingBar";

/*
 * 代码分割策略（A5）：
 * - FloatingBar：静态 import，不加载 Fluent 2，打包独立 chunk。
 * - MainLayout / OnboardingFlow：lazy import，包含 Fluent 2，按需加载。
 */
const MainLayout = lazy(
  () => import("./shared/components/MainLayout"),
);
const HomePage = lazy(
  () => import("./features/dashboard/HomePage"),
);
const HistoryPage = lazy(
  () => import("./features/history/HistoryPage"),
);
const DictionaryPage = lazy(
  () => import("./features/dictionary/DictionaryPage"),
);
const OnboardingFlow = lazy(
  () => import("./features/onboarding/OnboardingFlow"),
);

function App() {
  return (
    <ErrorBoundary>
      <Suspense fallback={
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100vh", background: "#fafafa" }}>
          <span style={{ color: "#888", fontSize: 14 }}>加载中…</span>
        </div>
      }>
        <Routes>
          {/* 悬浮录音条 — 轻量窗口，无 Fluent */}
          <Route path="/floating-bar" element={<FloatingBar />} />

          {/* 引导流程 */}
          <Route path="/onboarding" element={<OnboardingFlow />} />

          {/* 主窗口 Shell — Fluent 2 + 侧边栏导航 */}
          <Route path="/main" element={<MainLayout />}>
            <Route index element={<HomePage />} />
            <Route path="history" element={<HistoryPage />} />
            <Route path="dictionary" element={<DictionaryPage />} />
          </Route>
        </Routes>
      </Suspense>
    </ErrorBoundary>
  );
}

export default App;
