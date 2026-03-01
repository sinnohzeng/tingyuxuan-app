/**
 * 语言设置 section。
 *
 * 从 SettingsPanel.tsx 内联 LanguageConfig 迁移，
 * 替换 <select> 为 Fluent Dropdown + Option。
 */
import { Dropdown, Option, Field } from "@fluentui/react-components";
import type { AppConfig, ConfigUpdater } from "../../../shared/lib/types";

const PRIMARY_LANGUAGES = [
  { value: "auto", label: "自动检测" },
  { value: "zh", label: "中文" },
  { value: "en", label: "English" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
  { value: "fr", label: "Français" },
  { value: "de", label: "Deutsch" },
  { value: "es", label: "Español" },
];

const TARGET_LANGUAGES = PRIMARY_LANGUAGES.filter((l) => l.value !== "auto");

interface LanguageSectionProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
}

export default function LanguageSection({ config, updateConfig }: LanguageSectionProps) {
  return (
    <div className="flex flex-col gap-4">
      <Field label="主要听写语言">
        <Dropdown
          value={PRIMARY_LANGUAGES.find((l) => l.value === config.language.primary)?.label ?? ""}
          selectedOptions={[config.language.primary]}
          onOptionSelect={(_, data) => {
            if (!data.optionValue) return;
            updateConfig((c) => ({
              ...c,
              language: { ...c.language, primary: data.optionValue! },
            }));
          }}
        >
          {PRIMARY_LANGUAGES.map((l) => (
            <Option key={l.value} value={l.value}>{l.label}</Option>
          ))}
        </Dropdown>
      </Field>

      <Field label="翻译目标语言">
        <Dropdown
          value={TARGET_LANGUAGES.find((l) => l.value === config.language.translation_target)?.label ?? ""}
          selectedOptions={[config.language.translation_target]}
          onOptionSelect={(_, data) => {
            if (!data.optionValue) return;
            updateConfig((c) => ({
              ...c,
              language: { ...c.language, translation_target: data.optionValue! },
            }));
          }}
        >
          {TARGET_LANGUAGES.map((l) => (
            <Option key={l.value} value={l.value}>{l.label}</Option>
          ))}
        </Dropdown>
      </Field>
    </div>
  );
}
