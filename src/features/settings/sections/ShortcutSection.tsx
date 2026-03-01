/**
 * 快捷键展示 section — MVP 阶段快捷键固定，仅做只读展示。
 */
import { Text } from "@fluentui/react-components";

const SHORTCUTS = [
  { label: "听写开始/停止", key: "RAlt", description: "按下开始录音，再按停止" },
  { label: "翻译模式", key: "Shift + RAlt", description: "启动翻译模式录音" },
  { label: "AI 助手", key: "Alt + Space", description: "启动 AI 助手模式" },
  { label: "取消录音", key: "Esc", description: "取消当前录音" },
];

export default function ShortcutSection() {
  return (
    <div className="flex flex-col gap-3">
      <Text size={200} className="text-gray-500">
        快捷键当前为固定配置，后续版本将支持自定义。
      </Text>
      <div className="flex flex-col gap-2">
        {SHORTCUTS.map((item) => (
          <div key={item.label} className="flex items-center justify-between py-1.5">
            <div className="flex flex-col">
              <Text weight="semibold" size={300}>{item.label}</Text>
              <Text size={200} className="text-gray-500">{item.description}</Text>
            </div>
            <code className="px-2 py-1 bg-gray-100 dark:bg-gray-800 rounded text-sm shrink-0">
              {item.key}
            </code>
          </div>
        ))}
      </div>
    </div>
  );
}
