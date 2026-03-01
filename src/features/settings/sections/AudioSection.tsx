/**
 * 音频设置 section — 声音反馈开关。
 *
 * 从 GeneralConfig.tsx 提取，使用 Fluent Switch。
 */
import { Switch, Field } from "@fluentui/react-components";
import type { AppConfig, ConfigUpdater } from "../../../shared/lib/types";

interface AudioSectionProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
}

export default function AudioSection({ config, updateConfig }: AudioSectionProps) {
  return (
    <Field label="声音反馈" hint="录音开始和结束时播放提示音">
      <Switch
        checked={config.general.sound_feedback}
        onChange={(_, data) =>
          updateConfig((c) => ({
            ...c,
            general: { ...c.general, sound_feedback: data.checked },
          }))
        }
      />
    </Field>
  );
}
