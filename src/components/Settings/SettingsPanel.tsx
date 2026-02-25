import { useState, useEffect, useCallback } from "react";
import { useAppStore } from "../../stores/appStore";
import type { AppConfig } from "../../lib/types";
import ApiConfig from "./ApiConfig";
import ShortcutConfig from "./ShortcutConfig";
import GeneralConfig from "./GeneralConfig";
import DictionaryConfig from "./DictionaryConfig";
import HistoryPanel from "./HistoryPanel";
import SetupWizard from "./SetupWizard";

type Tab = "general" | "api" | "shortcuts" | "language" | "dictionary" | "history";

const TABS: { id: Tab; label: string }[] = [
  { id: "general", label: "常规" },
  { id: "api", label: "API 配置" },
  { id: "shortcuts", label: "快捷键" },
  { id: "language", label: "语言" },
  { id: "dictionary", label: "个人词典" },
  { id: "history", label: "历史记录" },
];

export default function SettingsPanel() {
  const [activeTab, setActiveTab] = useState<Tab>("api");
  const { config, setConfig } = useAppStore();
  const [localConfig, setLocalConfig] = useState<AppConfig | null>(null);
  const [saveStatus, setSaveStatus] = useState<string>("");
  const [showWizard, setShowWizard] = useState(false);

  // Listen for open-history event from tray menu.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    import("@tauri-apps/api/event")
      .then(({ listen }) => {
        listen("open-history", () => {
          setActiveTab("history");
          setShowWizard(false);
        }).then((u) => {
          unlisten = u;
        });
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Load config on mount and check if first launch.
  useEffect(() => {
    async function loadConfig() {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const cfg = await invoke<AppConfig>("get_config");
        setConfig(cfg);
        setLocalConfig(cfg);

        // Check if this is the first launch (no API keys configured).
        const isFirst = await invoke<boolean>("is_first_launch");
        if (isFirst) setShowWizard(true);
      } catch {
        // Dev mode fallback
        const defaultConfig: AppConfig = {
          general: {
            auto_launch: true,
            sound_feedback: true,
            floating_bar_position: "bottom_center",
          },
          shortcuts: {
            dictate: "alt_right",
            translate: "shift+alt_right",
            ai_assistant: "alt+space",
            cancel: "escape",
          },
          language: {
            primary: "auto",
            translation_target: "en",
            variant: null,
          },
          stt: {
            provider: "whisper",
            api_key_ref: "",
            base_url: null,
            model: "whisper-1",
          },
          llm: {
            provider: "openai",
            api_key_ref: "",
            base_url: null,
            model: "gpt-4o-mini",
          },
          cache: {
            audio_retention_hours: 24,
            failed_retention_days: 7,
            max_cache_size_mb: 500,
          },
          user_dictionary: [],
        };
        setLocalConfig(defaultConfig);
      }
    }
    loadConfig();
  }, [setConfig]);

  const handleSave = useCallback(async () => {
    if (!localConfig) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_config", { config: localConfig });
      setConfig(localConfig);
      setSaveStatus("已保存");
      setTimeout(() => setSaveStatus(""), 2000);
    } catch {
      setSaveStatus("保存失败（开发模式）");
      setTimeout(() => setSaveStatus(""), 2000);
    }
  }, [localConfig, setConfig]);

  const updateConfig = useCallback(
    (updater: (prev: AppConfig) => AppConfig) => {
      setLocalConfig((prev) => (prev ? updater(prev) : prev));
    },
    []
  );

  if (!localConfig) {
    return (
      <div className="flex items-center justify-center h-screen bg-gray-50">
        <div className="text-gray-400">加载配置中...</div>
      </div>
    );
  }

  if (showWizard) {
    return (
      <SetupWizard
        config={localConfig}
        onUpdate={updateConfig}
        onComplete={() => setShowWizard(false)}
      />
    );
  }

  return (
    <div className="flex flex-col h-screen bg-gray-50">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b bg-white">
        <h1 className="text-lg font-semibold text-gray-800">听语轩 设置</h1>
        <div className="flex items-center gap-3">
          {saveStatus && (
            <span className="text-sm text-green-600">{saveStatus}</span>
          )}
          <button
            onClick={handleSave}
            className="px-4 py-1.5 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 transition-colors"
          >
            保存
          </button>
        </div>
      </div>

      {/* Tab bar */}
      <div className="flex border-b bg-white px-6">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors ${
              activeTab === tab.id
                ? "border-blue-500 text-blue-600"
                : "border-transparent text-gray-500 hover:text-gray-700"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto p-6">
        {activeTab === "general" && (
          <GeneralConfig config={localConfig} onUpdate={updateConfig} />
        )}
        {activeTab === "api" && (
          <ApiConfig config={localConfig} onUpdate={updateConfig} />
        )}
        {activeTab === "shortcuts" && (
          <ShortcutConfig config={localConfig} onUpdate={updateConfig} />
        )}
        {activeTab === "language" && (
          <LanguageConfig config={localConfig} onUpdate={updateConfig} />
        )}
        {activeTab === "dictionary" && <DictionaryConfig />}
        {activeTab === "history" && <HistoryPanel />}
      </div>
    </div>
  );
}

/** Language settings tab */
function LanguageConfig({
  config,
  onUpdate,
}: {
  config: AppConfig;
  onUpdate: (updater: (prev: AppConfig) => AppConfig) => void;
}) {
  return (
    <div className="space-y-6 max-w-lg">
      <h2 className="text-base font-medium text-gray-700">语言设置</h2>

      <div>
        <label className="block text-sm text-gray-600 mb-1">主要听写语言</label>
        <select
          value={config.language.primary}
          onChange={(e) =>
            onUpdate((c) => ({
              ...c,
              language: { ...c.language, primary: e.target.value },
            }))
          }
          className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
        >
          <option value="auto">自动检测</option>
          <option value="zh">中文</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
          <option value="fr">Français</option>
          <option value="de">Deutsch</option>
          <option value="es">Español</option>
        </select>
      </div>

      <div>
        <label className="block text-sm text-gray-600 mb-1">翻译目标语言</label>
        <select
          value={config.language.translation_target}
          onChange={(e) =>
            onUpdate((c) => ({
              ...c,
              language: { ...c.language, translation_target: e.target.value },
            }))
          }
          className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
        >
          <option value="en">English</option>
          <option value="zh">中文</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
          <option value="fr">Français</option>
          <option value="de">Deutsch</option>
          <option value="es">Español</option>
        </select>
      </div>
    </div>
  );
}
