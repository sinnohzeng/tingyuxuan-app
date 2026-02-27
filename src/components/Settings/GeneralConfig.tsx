import type { AppConfig, FloatingBarPosition } from "../../lib/types";

interface GeneralConfigProps {
  config: AppConfig;
  onUpdate: (updater: (prev: AppConfig) => AppConfig) => void;
}

export default function GeneralConfig({ config, onUpdate }: GeneralConfigProps) {
  return (
    <div className="space-y-6 max-w-lg">
      <h2 className="text-base font-medium text-gray-700">常规设置</h2>

      {/* Toggle options */}
      <div className="space-y-4">
        <ToggleItem
          label="开机自启动"
          description="系统启动时自动运行听语轩"
          checked={config.general.auto_launch}
          onChange={(v) =>
            onUpdate((c) => ({
              ...c,
              general: { ...c.general, auto_launch: v },
            }))
          }
        />

        <ToggleItem
          label="声音反馈"
          description="录音开始和结束时播放提示音"
          checked={config.general.sound_feedback}
          onChange={(v) =>
            onUpdate((c) => ({
              ...c,
              general: { ...c.general, sound_feedback: v },
            }))
          }
        />
      </div>

      {/* Floating bar position */}
      <div>
        <label htmlFor="floating-bar-position" className="block text-sm font-medium text-gray-700 mb-1">
          浮动条位置
        </label>
        <select
          id="floating-bar-position"
          value={config.general.floating_bar_position}
          onChange={(e) =>
            onUpdate((c) => ({
              ...c,
              general: {
                ...c.general,
                floating_bar_position: e.target.value as FloatingBarPosition,
              },
            }))
          }
          className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
        >
          <option value="bottom_center">底部居中</option>
          <option value="follow_cursor">跟随光标</option>
          <option value="fixed">固定位置</option>
        </select>
      </div>

      {/* Cache settings */}
      <div>
        <h3 className="text-sm font-medium text-gray-700 mb-3">缓存管理</h3>

        <div className="space-y-3">
          <div>
            <label htmlFor="audio-retention" className="block text-xs text-gray-600 mb-1">
              音频缓存保留时长
            </label>
            <select
              id="audio-retention"
              value={config.cache.audio_retention_hours}
              onChange={(e) =>
                onUpdate((c) => ({
                  ...c,
                  cache: {
                    ...c.cache,
                    audio_retention_hours: parseInt(e.target.value),
                  },
                }))
              }
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
            >
              <option value={1}>1 小时</option>
              <option value={12}>12 小时</option>
              <option value={24}>24 小时</option>
              <option value={168}>7 天</option>
              <option value={0}>永久保留</option>
            </select>
          </div>

          <div>
            <label htmlFor="max-cache-size" className="block text-xs text-gray-600 mb-1">
              缓存最大体积 (MB)
            </label>
            <input
              id="max-cache-size"
              type="number"
              value={config.cache.max_cache_size_mb}
              onChange={(e) =>
                onUpdate((c) => ({
                  ...c,
                  cache: {
                    ...c.cache,
                    max_cache_size_mb: parseInt(e.target.value) || 500,
                  },
                }))
              }
              min={100}
              max={10000}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-blue-500"
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function ToggleItem({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between">
      <div>
        <div className="text-sm font-medium text-gray-700">{label}</div>
        <div className="text-xs text-gray-500">{description}</div>
      </div>
      <button
        onClick={() => onChange(!checked)}
        className={`relative w-11 h-6 rounded-full transition-colors ${
          checked ? "bg-blue-500" : "bg-gray-300"
        }`}
      >
        <span
          className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform ${
            checked ? "translate-x-5" : "translate-x-0"
          }`}
        />
      </button>
    </div>
  );
}
