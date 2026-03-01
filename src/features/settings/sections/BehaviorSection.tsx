/**
 * 行为设置 section — 自动启动 + 悬浮条位置 + 历史保留。
 *
 * 从 GeneralConfig.tsx 迁移，使用 Fluent Switch + Dropdown。
 */
import { Switch, Dropdown, Option, Field } from "@fluentui/react-components";
import type { AppConfig, ConfigUpdater, FloatingBarPosition } from "../../../shared/lib/types";

const POSITION_OPTIONS: { value: FloatingBarPosition; label: string }[] = [
  { value: "bottom_center", label: "底部居中" },
  { value: "follow_cursor", label: "跟随光标" },
  { value: "fixed", label: "固定位置" },
];

const RETENTION_OPTIONS = [
  { value: 7, label: "7 天" },
  { value: 30, label: "30 天" },
  { value: 90, label: "90 天" },
  { value: 365, label: "1 年" },
  { value: 0, label: "永久保留" },
];

interface BehaviorSectionProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
}

export default function BehaviorSection({ config, updateConfig }: BehaviorSectionProps) {
  return (
    <div className="flex flex-col gap-4">
      <Field label="开机自启动" hint="系统启动时自动运行听语轩">
        <Switch
          checked={config.general.auto_launch}
          onChange={(_, data) =>
            updateConfig((c) => ({
              ...c,
              general: { ...c.general, auto_launch: data.checked },
            }))
          }
        />
      </Field>

      <Field label="浮动条位置">
        <Dropdown
          value={POSITION_OPTIONS.find((o) => o.value === config.general.floating_bar_position)?.label ?? ""}
          selectedOptions={[config.general.floating_bar_position]}
          onOptionSelect={(_, data) => {
            if (!data.optionValue) return;
            updateConfig((c) => ({
              ...c,
              general: {
                ...c.general,
                floating_bar_position: data.optionValue as FloatingBarPosition,
              },
            }));
          }}
        >
          {POSITION_OPTIONS.map((o) => (
            <Option key={o.value} value={o.value}>{o.label}</Option>
          ))}
        </Dropdown>
      </Field>

      <Field label="历史记录保留天数">
        <Dropdown
          value={RETENTION_OPTIONS.find((o) => o.value === config.cache.history_retention_days)?.label ?? ""}
          selectedOptions={[String(config.cache.history_retention_days)]}
          onOptionSelect={(_, data) => {
            if (!data.optionValue) return;
            updateConfig((c) => ({
              ...c,
              cache: { ...c.cache, history_retention_days: parseInt(data.optionValue!) },
            }));
          }}
        >
          {RETENTION_OPTIONS.map((o) => (
            <Option key={o.value} value={String(o.value)}>{o.label}</Option>
          ))}
        </Dropdown>
      </Field>
    </div>
  );
}
