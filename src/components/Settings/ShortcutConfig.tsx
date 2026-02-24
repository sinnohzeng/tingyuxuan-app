import type { AppConfig } from "../../lib/types";

interface ShortcutConfigProps {
  config: AppConfig;
  onUpdate: (updater: (prev: AppConfig) => AppConfig) => void;
}

const SHORTCUT_ITEMS = [
  { key: "dictate" as const, label: "听写开始/停止", description: "按下开始录音，再按停止" },
  { key: "translate" as const, label: "翻译模式", description: "启动翻译模式录音" },
  { key: "ai_assistant" as const, label: "AI 助手", description: "启动 AI 助手模式" },
  { key: "cancel" as const, label: "取消录音", description: "取消当前录音" },
];

export default function ShortcutConfig({ config, onUpdate }: ShortcutConfigProps) {
  return (
    <div className="space-y-6 max-w-lg">
      <h2 className="text-base font-medium text-gray-700">键盘快捷键</h2>
      <p className="text-sm text-gray-500">
        自定义快捷键绑定。使用格式如 <code className="bg-gray-100 px-1 rounded">ctrl+shift+d</code>
      </p>

      <div className="space-y-4">
        {SHORTCUT_ITEMS.map((item) => (
          <div key={item.key} className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium text-gray-700">{item.label}</div>
              <div className="text-xs text-gray-500">{item.description}</div>
            </div>
            <input
              type="text"
              value={config.shortcuts[item.key]}
              onChange={(e) =>
                onUpdate((c) => ({
                  ...c,
                  shortcuts: { ...c.shortcuts, [item.key]: e.target.value },
                }))
              }
              className="w-40 px-3 py-1.5 border border-gray-300 rounded-lg text-sm text-center
                         font-mono focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            />
          </div>
        ))}
      </div>
    </div>
  );
}
