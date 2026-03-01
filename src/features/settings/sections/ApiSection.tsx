/**
 * API 配置 section — 薄编排层。
 *
 * 组合预设按钮 + provider/model/url 配置 + API key + 连接测试。
 */
import { Dropdown, Option, Input, Button, Text, Field, Divider } from "@fluentui/react-components";
import type { AppConfig, ConfigUpdater, LLMProviderType } from "../../../shared/lib/types";
import { PROVIDER_PRESETS } from "../../../shared/lib/providers";
import ApiKeyField from "../components/ApiKeyField";
import ConnectionTestButton from "../components/ConnectionTestButton";

interface ApiSectionProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
}

export default function ApiSection({ config, updateConfig }: ApiSectionProps) {
  return (
    <div className="flex flex-col gap-6">
      <PresetButtons updateConfig={updateConfig} />
      <Divider />
      <LlmBlock config={config} updateConfig={updateConfig} />
    </div>
  );
}

/** 快速配置预设按钮 */
function PresetButtons({ updateConfig }: { updateConfig: ConfigUpdater }) {
  const applyPreset = (key: string) => {
    const preset = PROVIDER_PRESETS[key];
    if (!preset) return;
    updateConfig((c) => ({
      ...c,
      llm: { ...c.llm, provider: preset.provider, base_url: preset.base_url, model: preset.models[0] },
    }));
  };

  return (
    <div className="flex flex-col gap-2">
      <Text weight="semibold">快速配置</Text>
      <div className="flex gap-2 flex-wrap">
        {Object.entries(PROVIDER_PRESETS).map(([key, preset]) => (
          <Button key={key} appearance="secondary" size="small" onClick={() => applyPreset(key)}>
            {preset.name}
          </Button>
        ))}
      </div>
    </div>
  );
}

/** 大语言模型 (LLM) 配置块 */
function LlmBlock({ config, updateConfig }: ApiSectionProps) {
  const LLM_OPTIONS = [
    { value: "dashscope", label: "阿里云 DashScope" },
    { value: "openai", label: "OpenAI" },
    { value: "volcengine", label: "火山引擎 (豆包)" },
    { value: "custom", label: "自定义（OpenAI 兼容）" },
  ];

  return (
    <div className="flex flex-col gap-3">
      <Text weight="semibold">多模态大模型</Text>
      <Text size={200} className="text-gray-500">请选择支持音频输入的模型</Text>

      <Field label="Provider">
        <Dropdown
          value={LLM_OPTIONS.find((o) => o.value === config.llm.provider)?.label ?? ""}
          selectedOptions={[config.llm.provider]}
          onOptionSelect={(_, data) => {
            if (!data.optionValue) return;
            updateConfig((c) => ({
              ...c,
              llm: { ...c.llm, provider: data.optionValue as LLMProviderType },
            }));
          }}
        >
          {LLM_OPTIONS.map((o) => (
            <Option key={o.value} value={o.value}>{o.label}</Option>
          ))}
        </Dropdown>
      </Field>

      <ApiKeyField service="llm" label="API Key" />

      <Field label="Base URL">
        <Input
          value={config.llm.base_url ?? ""}
          onChange={(_, data) =>
            updateConfig((c) => ({ ...c, llm: { ...c.llm, base_url: data.value || null } }))
          }
          placeholder="默认值由 Provider 决定"
        />
      </Field>

      <Field label="Model">
        <Input
          value={config.llm.model}
          onChange={(_, data) =>
            updateConfig((c) => ({ ...c, llm: { ...c.llm, model: data.value } }))
          }
          placeholder="例如 qwen3-omni-flash"
        />
      </Field>

      <ConnectionTestButton command="test_llm_connection" label="测试连接" />
    </div>
  );
}
