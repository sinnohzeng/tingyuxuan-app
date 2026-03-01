/**
 * API Key 输入组件 — 掩码显示 + 编辑模式。
 *
 * 已配置：显示掩码 + "已配置" Badge + "更新" 按钮
 * 未配置/编辑中：显示输入框 + "保存" 按钮
 */
import { Input, Button, Badge, Field, Spinner } from "@fluentui/react-components";
import { useApiKey } from "../hooks/useApiKey";

interface ApiKeyFieldProps {
  service: "llm";
  label: string;
}

export default function ApiKeyField({ service, label }: ApiKeyFieldProps) {
  const {
    keyValue, maskedKey, keyStatus, isEditing,
    setKeyValue, startEditing, cancelEditing, saveKey,
  } = useApiKey(service);

  if (keyStatus === "loading") {
    return (
      <Field label={label}>
        <Spinner size="tiny" label="加载中..." />
      </Field>
    );
  }

  // 已配置 + 未编辑：显示掩码
  if (keyStatus === "configured" && !isEditing) {
    return (
      <Field label={label}>
        <div className="flex items-center gap-2">
          <code className="flex-1 px-3 py-1.5 bg-gray-100 dark:bg-gray-800 rounded text-sm text-gray-500">
            {maskedKey}
          </code>
          <Badge appearance="filled" color="success">已配置</Badge>
          <Button appearance="secondary" size="small" onClick={startEditing}>
            更新
          </Button>
        </div>
      </Field>
    );
  }

  // 未配置 or 编辑中：显示输入框
  return (
    <Field
      label={label}
      validationState={keyStatus === "save_failed" ? "error" : "none"}
      validationMessage={keyStatus === "save_failed" ? "保存失败，请重试" : undefined}
    >
      <div className="flex gap-2">
        <Input
          className="flex-1"
          type="password"
          value={keyValue}
          onChange={(_, data) => setKeyValue(data.value)}
          placeholder={isEditing ? "输入新 Key 以更新" : `输入 ${label}`}
          onKeyDown={(e) => { if (e.key === "Enter") saveKey(); }}
        />
        <Button
          appearance="primary"
          disabled={!keyValue.trim()}
          onClick={saveKey}
        >
          保存
        </Button>
        {isEditing && (
          <Button appearance="secondary" onClick={cancelEditing}>
            取消
          </Button>
        )}
      </div>
    </Field>
  );
}
