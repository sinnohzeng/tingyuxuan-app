/**
 * 快捷键配置 section。
 *
 * 从 ShortcutConfig.tsx 迁移，替换原生 HTML 为 Fluent 2 组件。
 * 保留 SHORTCUT_PATTERN 验证 + 两层状态管理。
 */
import { useState } from "react";
import { Input, Text, Field } from "@fluentui/react-components";
import type { AppConfig, ConfigUpdater } from "../../../shared/lib/types";

const SHORTCUT_ITEMS = [
  { key: "dictate" as const, label: "听写开始/停止", description: "按下开始录音，再按停止" },
  { key: "translate" as const, label: "翻译模式", description: "启动翻译模式录音" },
  { key: "ai_assistant" as const, label: "AI 助手", description: "启动 AI 助手模式" },
  { key: "cancel" as const, label: "取消录音", description: "取消当前录音" },
];

const SHORTCUT_PATTERN = /^[a-z][a-z0-9_]*(\+[a-z][a-z0-9_]*)*$/i;
type ShortcutKey = (typeof SHORTCUT_ITEMS)[number]["key"];

interface ShortcutSectionProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
}

export default function ShortcutSection({ config, updateConfig }: ShortcutSectionProps) {
  const [invalidKeys, setInvalidKeys] = useState<Set<string>>(new Set());
  const [localEdits, setLocalEdits] = useState<Partial<Record<ShortcutKey, string>>>({});

  const handleChange = (key: ShortcutKey, val: string) => {
    const valid = val === "" || SHORTCUT_PATTERN.test(val);
    setInvalidKeys((prev) => {
      const next = new Set(prev);
      valid ? next.delete(key) : next.add(key);
      return next;
    });
    if (valid) {
      setLocalEdits((prev) => { const next = { ...prev }; delete next[key]; return next; });
      updateConfig((c) => ({ ...c, shortcuts: { ...c.shortcuts, [key]: val } }));
    } else {
      setLocalEdits((prev) => ({ ...prev, [key]: val }));
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <Text size={200}>
        自定义快捷键绑定。格式如 <code>alt_right</code>、<code>shift+alt_right</code>
      </Text>
      {SHORTCUT_ITEMS.map((item) => (
        <Field
          key={item.key}
          label={item.label}
          hint={item.description}
          validationMessage={invalidKeys.has(item.key) ? "格式无效" : undefined}
          validationState={invalidKeys.has(item.key) ? "error" : "none"}
        >
          <Input
            value={localEdits[item.key] ?? config.shortcuts[item.key]}
            onChange={(_, data) => handleChange(item.key, data.value)}
            style={{ fontFamily: "monospace" }}
          />
        </Field>
      ))}
    </div>
  );
}
