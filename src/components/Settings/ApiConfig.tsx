import { useState, useEffect, useCallback, useRef } from "react";
import type { AppConfig, STTProviderType, LLMProviderType } from "../../lib/types";
import { PROVIDER_PRESETS } from "../../lib/providers";

interface ApiConfigProps {
  config: AppConfig;
  onUpdate: (updater: (prev: AppConfig) => AppConfig) => void;
}

export default function ApiConfig({ config, onUpdate }: ApiConfigProps) {
  const [sttApiKey, setSttApiKey] = useState("");
  const [llmApiKey, setLlmApiKey] = useState("");
  const [showSttKey, setShowSttKey] = useState(false);
  const [showLlmKey, setShowLlmKey] = useState(false);
  const [sttKeyStatus, setSttKeyStatus] = useState<string>("");
  const [llmKeyStatus, setLlmKeyStatus] = useState<string>("");
  const [sttTestResult, setSttTestResult] = useState<string>("");
  const [llmTestResult, setLlmTestResult] = useState<string>("");

  const sttKeyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const llmKeyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const sttTestTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const llmTestTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 清理定时器
  useEffect(() => {
    return () => {
      if (sttKeyTimerRef.current) clearTimeout(sttKeyTimerRef.current);
      if (llmKeyTimerRef.current) clearTimeout(llmKeyTimerRef.current);
      if (sttTestTimerRef.current) clearTimeout(sttTestTimerRef.current);
      if (llmTestTimerRef.current) clearTimeout(llmTestTimerRef.current);
    };
  }, []);

  // Check API key status on mount
  useEffect(() => {
    async function checkKeys() {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const sttKey = await invoke<string | null>("get_api_key", { service: "stt" });
        setSttKeyStatus(sttKey ? "已配置" : "未配置");
        const llmKey = await invoke<string | null>("get_api_key", { service: "llm" });
        setLlmKeyStatus(llmKey ? "已配置" : "未配置");
      } catch {
        // Dev mode
      }
    }
    checkKeys();
  }, []);

  const saveSttKey = useCallback(async () => {
    if (!sttApiKey.trim()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_api_key", { service: "stt", key: sttApiKey.trim() });
      setSttKeyStatus("已配置");
      setSttApiKey("");
    } catch {
      setSttKeyStatus("保存失败");
    }
    if (sttKeyTimerRef.current) clearTimeout(sttKeyTimerRef.current);
    sttKeyTimerRef.current = setTimeout(() => setSttKeyStatus((s) => s === "保存失败" ? "" : s), 3000);
  }, [sttApiKey]);

  const saveLlmKey = useCallback(async () => {
    if (!llmApiKey.trim()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_api_key", { service: "llm", key: llmApiKey.trim() });
      setLlmKeyStatus("已配置");
      setLlmApiKey("");
    } catch {
      setLlmKeyStatus("保存失败");
    }
    if (llmKeyTimerRef.current) clearTimeout(llmKeyTimerRef.current);
    llmKeyTimerRef.current = setTimeout(() => setLlmKeyStatus((s) => s === "保存失败" ? "" : s), 3000);
  }, [llmApiKey]);

  const applyPreset = (presetKey: string) => {
    const preset = PROVIDER_PRESETS[presetKey];
    if (!preset) return;

    onUpdate((c) => ({
      ...c,
      llm: {
        ...c.llm,
        provider: preset.llm_provider,
        base_url: preset.llm_base_url,
        model: preset.llm_models[0],
      },
      stt: {
        ...c.stt,
        provider: preset.stt_provider,
        base_url: preset.stt_base_url,
        model: preset.stt_model,
      },
    }));
  };

  const testSttConnection = async () => {
    setSttTestResult("测试中...");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const ok = await invoke<boolean>("test_stt_connection");
      setSttTestResult(ok ? "连接成功" : "连接失败");
    } catch {
      setSttTestResult("测试失败（开发模式）");
    }
    if (sttTestTimerRef.current) clearTimeout(sttTestTimerRef.current);
    sttTestTimerRef.current = setTimeout(() => setSttTestResult(""), 3000);
  };

  const testLlmConnection = async () => {
    setLlmTestResult("测试中...");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const ok = await invoke<boolean>("test_llm_connection");
      setLlmTestResult(ok ? "连接成功" : "连接失败");
    } catch {
      setLlmTestResult("测试失败（开发模式）");
    }
    if (llmTestTimerRef.current) clearTimeout(llmTestTimerRef.current);
    llmTestTimerRef.current = setTimeout(() => setLlmTestResult(""), 3000);
  };

  return (
    <div className="space-y-8 max-w-lg">
      {/* Provider presets */}
      <div>
        <h2 className="text-base font-medium text-gray-700 mb-3">快速配置</h2>
        <div className="flex gap-2 flex-wrap">
          {Object.entries(PROVIDER_PRESETS).map(([key, preset]) => (
            <button
              key={key}
              onClick={() => applyPreset(key)}
              className="px-3 py-1.5 text-sm border border-gray-300 rounded-lg
                         hover:bg-blue-50 hover:border-blue-300 transition-colors"
            >
              {preset.name}
            </button>
          ))}
        </div>
      </div>

      {/* STT Configuration */}
      <div className="space-y-3">
        <h2 className="text-base font-medium text-gray-700">语音识别 (STT)</h2>

        <div>
          <label htmlFor="stt-provider" className="block text-sm text-gray-600 mb-1">Provider</label>
          <select
            id="stt-provider"
            value={config.stt.provider}
            onChange={(e) =>
              onUpdate((c) => ({
                ...c,
                stt: { ...c.stt, provider: e.target.value as STTProviderType },
              }))
            }
            className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
          >
            <option value="dashscope_streaming">阿里云 DashScope（流式）</option>
          </select>
        </div>

        <div>
          <label className="block text-sm text-gray-600 mb-1">
            API Key
            {sttKeyStatus && (
              <span className={`ml-2 text-xs ${sttKeyStatus === "已配置" ? "text-green-600" : "text-gray-400"}`}>
                ({sttKeyStatus})
              </span>
            )}
          </label>
          <div className="flex gap-2">
            <input
              type={showSttKey ? "text" : "password"}
              value={sttApiKey}
              onChange={(e) => setSttApiKey(e.target.value)}
              placeholder={sttKeyStatus === "已配置" ? "输入新 Key 以更新" : "输入 STT API Key"}
              className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
              onKeyDown={(e) => { if (e.key === "Enter") saveSttKey(); }}
            />
            <button
              onClick={() => setShowSttKey(!showSttKey)}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm text-gray-500 hover:bg-gray-50"
            >
              {showSttKey ? "隐藏" : "显示"}
            </button>
            <button
              onClick={saveSttKey}
              disabled={!sttApiKey.trim()}
              className="px-3 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600
                         disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              保存
            </button>
          </div>
        </div>

        <div>
          <label htmlFor="stt-base-url" className="block text-sm text-gray-600 mb-1">Base URL</label>
          <input
            id="stt-base-url"
            type="text"
            value={config.stt.base_url ?? ""}
            onChange={(e) =>
              onUpdate((c) => ({
                ...c,
                stt: { ...c.stt, base_url: e.target.value || null },
              }))
            }
            placeholder="默认值由 Provider 决定"
            className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div>
          <label htmlFor="stt-model" className="block text-sm text-gray-600 mb-1">Model</label>
          <input
            id="stt-model"
            type="text"
            value={config.stt.model ?? ""}
            onChange={(e) =>
              onUpdate((c) => ({
                ...c,
                stt: { ...c.stt, model: e.target.value || null },
              }))
            }
            placeholder="默认值由 Provider 决定"
            className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={testSttConnection}
            className="px-4 py-1.5 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
          >
            测试连接
          </button>
          {sttTestResult && (
            <span
              className={`text-sm ${
                sttTestResult.includes("成功") ? "text-green-600" : "text-gray-500"
              }`}
            >
              {sttTestResult}
            </span>
          )}
        </div>
      </div>

      {/* LLM Configuration */}
      <div className="space-y-3">
        <h2 className="text-base font-medium text-gray-700">大语言模型 (LLM)</h2>

        <div>
          <label htmlFor="llm-provider" className="block text-sm text-gray-600 mb-1">Provider</label>
          <select
            id="llm-provider"
            value={config.llm.provider}
            onChange={(e) =>
              onUpdate((c) => ({
                ...c,
                llm: { ...c.llm, provider: e.target.value as LLMProviderType },
              }))
            }
            className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
          >
            <option value="openai">OpenAI</option>
            <option value="dashscope">阿里云 DashScope</option>
            <option value="volcengine">火山引擎 (豆包)</option>
            <option value="custom">自定义（OpenAI 兼容）</option>
          </select>
        </div>

        <div>
          <label className="block text-sm text-gray-600 mb-1">
            API Key
            {llmKeyStatus && (
              <span className={`ml-2 text-xs ${llmKeyStatus === "已配置" ? "text-green-600" : "text-gray-400"}`}>
                ({llmKeyStatus})
              </span>
            )}
          </label>
          <div className="flex gap-2">
            <input
              type={showLlmKey ? "text" : "password"}
              value={llmApiKey}
              onChange={(e) => setLlmApiKey(e.target.value)}
              placeholder={llmKeyStatus === "已配置" ? "输入新 Key 以更新" : "输入 LLM API Key"}
              className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
              onKeyDown={(e) => { if (e.key === "Enter") saveLlmKey(); }}
            />
            <button
              onClick={() => setShowLlmKey(!showLlmKey)}
              className="px-3 py-2 border border-gray-300 rounded-lg text-sm text-gray-500 hover:bg-gray-50"
            >
              {showLlmKey ? "隐藏" : "显示"}
            </button>
            <button
              onClick={saveLlmKey}
              disabled={!llmApiKey.trim()}
              className="px-3 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600
                         disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              保存
            </button>
          </div>
        </div>

        <div>
          <label htmlFor="llm-base-url" className="block text-sm text-gray-600 mb-1">Base URL</label>
          <input
            id="llm-base-url"
            type="text"
            value={config.llm.base_url ?? ""}
            onChange={(e) =>
              onUpdate((c) => ({
                ...c,
                llm: { ...c.llm, base_url: e.target.value || null },
              }))
            }
            placeholder="默认值由 Provider 决定"
            className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div>
          <label htmlFor="llm-model" className="block text-sm text-gray-600 mb-1">Model</label>
          <input
            id="llm-model"
            type="text"
            value={config.llm.model}
            onChange={(e) =>
              onUpdate((c) => ({
                ...c,
                llm: { ...c.llm, model: e.target.value },
              }))
            }
            placeholder="例如 gpt-4o-mini"
            className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={testLlmConnection}
            className="px-4 py-1.5 text-sm border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
          >
            测试连接
          </button>
          {llmTestResult && (
            <span
              className={`text-sm ${
                llmTestResult.includes("成功") ? "text-green-600" : "text-gray-500"
              }`}
            >
              {llmTestResult}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
