/**
 * 设置 Tab — 聚合 5 个 section，可滚动容器。
 *
 * 每个 section < 100 行，无需 lazy loading。
 */
import { Divider, Title3 } from "@fluentui/react-components";
import type { AppConfig, ConfigUpdater } from "../../../shared/lib/types";
import ShortcutSection from "../sections/ShortcutSection";
import LanguageSection from "../sections/LanguageSection";
import AudioSection from "../sections/AudioSection";
import BehaviorSection from "../sections/BehaviorSection";
import ApiSection from "../sections/ApiSection";

interface SettingsTabProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
}

export default function SettingsTab({ config, updateConfig }: SettingsTabProps) {
  return (
    <div className="flex flex-col gap-6 py-4">
      <section>
        <Title3 className="mb-3">API 配置</Title3>
        <ApiSection config={config} updateConfig={updateConfig} />
      </section>

      <Divider />

      <section>
        <Title3 className="mb-3">快捷键</Title3>
        <ShortcutSection config={config} updateConfig={updateConfig} />
      </section>

      <Divider />

      <section>
        <Title3 className="mb-3">语言</Title3>
        <LanguageSection config={config} updateConfig={updateConfig} />
      </section>

      <Divider />

      <section>
        <Title3 className="mb-3">音频</Title3>
        <AudioSection config={config} updateConfig={updateConfig} />
      </section>

      <Divider />

      <section>
        <Title3 className="mb-3">行为</Title3>
        <BehaviorSection config={config} updateConfig={updateConfig} />
      </section>
    </div>
  );
}
