import { useState } from "react";
import ErrorBoundary from "./components/ErrorBoundary";
import FloatingBar from "./components/FloatingBar";
import SettingsPanel from "./components/Settings/SettingsPanel";

function getInitialRoute(): string {
  const path = window.location.pathname;
  if (path.includes("floating-bar")) return "floating-bar";
  if (path.includes("settings")) return "settings";
  return "floating-bar";
}

function AppContent() {
  const [route, setRoute] = useState(getInitialRoute);

  switch (route) {
    case "floating-bar":
      return <FloatingBar />;
    case "settings":
      return <SettingsPanel />;
    default:
      return (
        <div className="p-4">
          <h1 className="text-xl font-bold mb-4">听语轩 TingYuXuan</h1>
          <p className="text-gray-600 mb-4">开发模式 - 选择视图：</p>
          <div className="flex gap-4">
            <button
              onClick={() => setRoute("floating-bar")}
              className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600"
            >
              浮动状态条
            </button>
            <button
              onClick={() => setRoute("settings")}
              className="px-4 py-2 bg-gray-500 text-white rounded-lg hover:bg-gray-600"
            >
              设置面板
            </button>
          </div>
        </div>
      );
  }
}

function App() {
  return (
    <ErrorBoundary>
      <AppContent />
    </ErrorBoundary>
  );
}

export default App;
