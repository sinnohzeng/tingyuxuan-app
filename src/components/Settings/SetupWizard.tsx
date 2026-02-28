import { useState, useCallback } from "react";
import { PROVIDER_PRESETS } from "../../lib/providers";
import type { AppConfig, STTProviderType, LLMProviderType } from "../../lib/types";

interface SetupWizardProps {
  config: AppConfig;
  onUpdate: (updater: (prev: AppConfig) => AppConfig) => void;
  onComplete: () => void;
}

type Step = 1 | 2 | 3;

// 从 PROVIDER_PRESETS 动态生成，避免与已删除的 provider 不同步。
const PROVIDERS = Object.entries(PROVIDER_PRESETS).map(([id, preset]) => ({
  id,
  label: preset.name,
  desc: `${preset.llm_models[0]} + ${preset.stt_model ?? preset.stt_provider}`,
}));

export default function SetupWizard({ config, onUpdate, onComplete }: SetupWizardProps) {
  const [step, setStep] = useState<Step>(1);
  const [selectedProvider, setSelectedProvider] = useState<string>("dashscope");
  const [sttApiKey, setSttApiKey] = useState("");
  const [llmApiKey, setLlmApiKey] = useState("");
  const [showSttKey, setShowSttKey] = useState(false);
  const [showLlmKey, setShowLlmKey] = useState(false);
  const [sttTestResult, setSttTestResult] = useState<"idle" | "testing" | "success" | "failed">("idle");
  const [llmTestResult, setLlmTestResult] = useState<"idle" | "testing" | "success" | "failed">("idle");

  const isSameKey = selectedProvider === "dashscope" || selectedProvider === "openai";

  // Step 1 → Step 2: Apply provider preset.
  const handleProviderNext = useCallback(() => {
    const preset = PROVIDER_PRESETS[selectedProvider];
    if (preset) {
      onUpdate((c) => ({
        ...c,
        stt: {
          ...c.stt,
          provider: preset.stt_provider as STTProviderType,
          base_url: preset.stt_base_url,
          model: preset.stt_model,
        },
        llm: {
          ...c.llm,
          provider: selectedProvider as LLMProviderType,
          base_url: preset.llm_base_url,
          model: preset.llm_models[0],
        },
      }));
    }
    setStep(2);
  }, [selectedProvider, onUpdate]);

  // Step 2 → Step 3: Save keys and config.
  const handleKeysNext = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      if (sttApiKey.trim()) {
        await invoke("save_api_key", { service: "stt", key: sttApiKey.trim() });
      }
      if (llmApiKey.trim()) {
        await invoke("save_api_key", { service: "llm", key: llmApiKey.trim() });
      } else if (isSameKey && sttApiKey.trim()) {
        // For providers that share the same key.
        await invoke("save_api_key", { service: "llm", key: sttApiKey.trim() });
      }
    } catch (e) {
      console.warn("Tauri unavailable:", e);
    }
    setStep(3);
  }, [sttApiKey, llmApiKey, isSameKey]);

  // Step 3: Test connections.
  const runTests = useCallback(async () => {
    setSttTestResult("testing");
    setLlmTestResult("testing");

    try {
      const { invoke } = await import("@tauri-apps/api/core");

      // Save config first so pipeline can rebuild.
      await invoke("save_config", { config }).catch(() => {});

      try {
        await invoke("test_stt_connection");
        setSttTestResult("success");
      } catch {
        setSttTestResult("failed");
      }

      try {
        await invoke("test_llm_connection");
        setLlmTestResult("success");
      } catch {
        setLlmTestResult("failed");
      }
    } catch (e) {
      console.warn("Tauri unavailable:", e);
      setSttTestResult("failed");
      setLlmTestResult("failed");
    }
  }, [config]);

  // Complete wizard: save config and close.
  const handleComplete = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_config", { config });
    } catch (e) {
      console.warn("Tauri unavailable:", e);
    }
    onComplete();
  }, [config, onComplete]);

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-50 p-8">
      <div className="w-full max-w-md space-y-6">
        {/* Progress indicator */}
        <div className="flex items-center justify-center gap-2 mb-4">
          {[1, 2, 3].map((s) => (
            <div
              key={s}
              className={`w-2.5 h-2.5 rounded-full transition-colors ${
                s === step ? "bg-blue-500" : s < step ? "bg-blue-300" : "bg-gray-300"
              }`}
            />
          ))}
        </div>

        {/* Step 1: Select Provider */}
        {step === 1 && (
          <div className="space-y-6">
            <div className="text-center">
              <h2 className="text-xl font-semibold text-gray-800">
                欢迎使用听语轩
              </h2>
              <p className="text-sm text-gray-500 mt-2">
                请选择您的 API 服务商
              </p>
            </div>

            <div className="space-y-2">
              {PROVIDERS.map((p) => (
                <button
                  key={p.id}
                  onClick={() => setSelectedProvider(p.id)}
                  className={`w-full text-left px-4 py-3 rounded-lg border-2 transition-colors ${
                    selectedProvider === p.id
                      ? "border-blue-500 bg-blue-50"
                      : "border-gray-200 hover:border-gray-300"
                  }`}
                >
                  <div className="text-sm font-medium text-gray-800">
                    {p.label}
                  </div>
                  <div className="text-xs text-gray-500">{p.desc}</div>
                </button>
              ))}
            </div>

            <div className="flex justify-end">
              <button
                onClick={handleProviderNext}
                className="px-6 py-2 bg-blue-500 text-white text-sm rounded-lg
                           hover:bg-blue-600 transition-colors"
              >
                下一步
              </button>
            </div>
          </div>
        )}

        {/* Step 2: API Keys */}
        {step === 2 && (
          <div className="space-y-6">
            <div className="text-center">
              <h2 className="text-xl font-semibold text-gray-800">
                配置 API 密钥
              </h2>
              <p className="text-sm text-gray-500 mt-2">
                {isSameKey
                  ? "此服务商可使用同一个 API Key"
                  : "请分别输入 STT 和 LLM 的 API Key"}
              </p>
            </div>

            {/* STT API Key */}
            <div>
              <label htmlFor="wizard-stt-key" className="block text-sm text-gray-600 mb-1">
                {isSameKey ? "API Key" : "语音识别 (STT) API Key"}
              </label>
              <div className="relative">
                <input
                  id="wizard-stt-key"
                  type={showSttKey ? "text" : "password"}
                  value={sttApiKey}
                  onChange={(e) => setSttApiKey(e.target.value)}
                  placeholder="输入 API Key"
                  className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm
                             focus:ring-2 focus:ring-blue-500 focus:border-blue-500 pr-10"
                />
                <button
                  type="button"
                  onClick={() => setShowSttKey((s) => !s)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 text-xs"
                >
                  {showSttKey ? "隐藏" : "显示"}
                </button>
              </div>
            </div>

            {/* LLM API Key (only shown if different from STT) */}
            {!isSameKey && (
              <div>
                <label htmlFor="wizard-llm-key" className="block text-sm text-gray-600 mb-1">
                  大语言模型 (LLM) API Key
                </label>
                <div className="relative">
                  <input
                    id="wizard-llm-key"
                    type={showLlmKey ? "text" : "password"}
                    value={llmApiKey}
                    onChange={(e) => setLlmApiKey(e.target.value)}
                    placeholder="输入 API Key"
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm
                               focus:ring-2 focus:ring-blue-500 focus:border-blue-500 pr-10"
                  />
                  <button
                    type="button"
                    onClick={() => setShowLlmKey((s) => !s)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 text-xs"
                  >
                    {showLlmKey ? "隐藏" : "显示"}
                  </button>
                </div>
              </div>
            )}

            <div className="flex justify-between">
              <button
                onClick={() => setStep(1)}
                className="px-4 py-2 text-sm text-gray-500 hover:text-gray-700 transition-colors"
              >
                上一步
              </button>
              <button
                onClick={handleKeysNext}
                disabled={!sttApiKey.trim()}
                className="px-6 py-2 bg-blue-500 text-white text-sm rounded-lg
                           hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed
                           transition-colors"
              >
                下一步
              </button>
            </div>
          </div>
        )}

        {/* Step 3: Test Connection */}
        {step === 3 && (
          <div className="space-y-6">
            <div className="text-center">
              <h2 className="text-xl font-semibold text-gray-800">
                测试连接
              </h2>
              <p className="text-sm text-gray-500 mt-2">
                验证 API 配置是否正确
              </p>
            </div>

            <div className="space-y-3">
              <div className="flex items-center justify-between px-4 py-3 bg-white rounded-lg border border-gray-200">
                <span className="text-sm text-gray-700">STT 连接</span>
                <TestStatus status={sttTestResult} />
              </div>
              <div className="flex items-center justify-between px-4 py-3 bg-white rounded-lg border border-gray-200">
                <span className="text-sm text-gray-700">LLM 连接</span>
                <TestStatus status={llmTestResult} />
              </div>
            </div>

            {sttTestResult === "idle" && (
              <button
                onClick={runTests}
                className="w-full py-2 bg-blue-500 text-white text-sm rounded-lg
                           hover:bg-blue-600 transition-colors"
              >
                开始测试
              </button>
            )}

            <div className="flex justify-between">
              <button
                onClick={() => setStep(2)}
                className="px-4 py-2 text-sm text-gray-500 hover:text-gray-700 transition-colors"
              >
                上一步
              </button>
              <button
                onClick={handleComplete}
                className="px-6 py-2 bg-green-500 text-white text-sm rounded-lg
                           hover:bg-green-600 transition-colors"
              >
                完成
              </button>
            </div>
          </div>
        )}

        {/* Skip link */}
        <div className="text-center">
          <button
            onClick={onComplete}
            className="text-xs text-gray-400 hover:text-gray-600 transition-colors"
          >
            跳过引导，稍后手动配置
          </button>
        </div>
      </div>
    </div>
  );
}

function TestStatus({ status }: { status: "idle" | "testing" | "success" | "failed" }) {
  switch (status) {
    case "idle":
      return <span className="text-xs text-gray-400">待测试</span>;
    case "testing":
      return (
        <div className="flex items-center gap-1.5">
          <div className="w-3 h-3 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
          <span className="text-xs text-blue-500">测试中</span>
        </div>
      );
    case "success":
      return <span className="text-xs text-green-600">连接成功</span>;
    case "failed":
      return <span className="text-xs text-red-500">连接失败</span>;
  }
}
