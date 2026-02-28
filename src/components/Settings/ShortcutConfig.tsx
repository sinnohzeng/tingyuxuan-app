import { useState } from "react";
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

/** 校验快捷键格式：允许修饰键+键名组合，如 alt_right、shift+alt_right、ctrl+space */
const SHORTCUT_PATTERN = /^[a-z][a-z0-9_]*(\+[a-z][a-z0-9_]*)*$/i;

function isValidShortcut(value: string): boolean {
  return value === "" || SHORTCUT_PATTERN.test(value);
}

type ShortcutKey = (typeof SHORTCUT_ITEMS)[number]["key"];

export default function ShortcutConfig({ config, onUpdate }: ShortcutConfigProps) {
  const [invalidKeys, setInvalidKeys] = useState<Set<string>>(new Set());
  // 本地编辑状态：仅在输入无效时暂存值，有效时为 undefined（跟随 config）
  const [localEdits, setLocalEdits] = useState<Partial<Record<ShortcutKey, string>>>({});

  return (
    <div className="space-y-6 max-w-lg">
      <h2 className="text-base font-medium text-gray-700">键盘快捷键</h2>
      <p className="text-sm text-gray-500">
        自定义快捷键绑定。使用格式如 <code className="bg-gray-100 px-1 rounded">alt_right</code>、<code className="bg-gray-100 px-1 rounded">shift+alt_right</code>
      </p>

      <div className="space-y-4">
        {SHORTCUT_ITEMS.map((item) => {
          const isInvalid = invalidKeys.has(item.key);
          const displayValue = localEdits[item.key] ?? config.shortcuts[item.key];
          return (
            <div key={item.key} className="flex items-center justify-between">
              <div>
                <div className="text-sm font-medium text-gray-700">{item.label}</div>
                <div className="text-xs text-gray-500">{item.description}</div>
              </div>
              <div className="flex flex-col items-end">
                <input
                  type="text"
                  value={displayValue}
                  onChange={(e) => {
                    const val = e.target.value;
                    const valid = isValidShortcut(val);
                    setInvalidKeys((prev) => {
                      const next = new Set(prev);
                      if (valid) next.delete(item.key);
                      else next.add(item.key);
                      return next;
                    });
                    if (valid) {
                      // 有效：传播到 config 并清除本地编辑
                      setLocalEdits((prev) => {
                        const next = { ...prev };
                        delete next[item.key];
                        return next;
                      });
                      onUpdate((c) => ({
                        ...c,
                        shortcuts: { ...c.shortcuts, [item.key]: val },
                      }));
                    } else {
                      // 无效：仅更新本地显示，不传播到 config
                      setLocalEdits((prev) => ({ ...prev, [item.key]: val }));
                    }
                  }}
                  className={`w-40 px-3 py-1.5 border rounded-lg text-sm text-center
                             font-mono focus:ring-2 focus:ring-blue-500 focus:border-blue-500
                             ${isInvalid ? "border-red-400" : "border-gray-300"}`}
                />
                {isInvalid && (
                  <span className="text-xs text-red-500 mt-0.5">格式无效</span>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
