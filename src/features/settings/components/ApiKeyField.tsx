/**
 * 可复用的 API Key 输入组件。
 *
 * 内部使用 useApiKey hook 管理状态，对外仅需指定 service。
 */
import { Input, Button, Badge, Field } from "@fluentui/react-components";
import { EyeRegular, EyeOffRegular } from "@fluentui/react-icons";
import { useApiKey } from "../hooks/useApiKey";

interface ApiKeyFieldProps {
  service: "llm";
  label: string;
}

export default function ApiKeyField({ service, label }: ApiKeyFieldProps) {
  const { keyValue, showKey, keyStatus, setKeyValue, toggleShowKey, saveKey } = useApiKey(service);

  const placeholder = keyStatus === "已配置" ? "输入新 Key 以更新" : `输入 ${label} API Key`;

  return (
    <Field
      label={label}
      hint={keyStatus ? `状态: ${keyStatus}` : undefined}
      validationState={keyStatus === "保存失败" ? "error" : "none"}
    >
      <div className="flex gap-2">
        <Input
          className="flex-1"
          type={showKey ? "text" : "password"}
          value={keyValue}
          onChange={(_, data) => setKeyValue(data.value)}
          placeholder={placeholder}
          onKeyDown={(e) => { if (e.key === "Enter") saveKey(); }}
          contentAfter={
            <Button
              appearance="transparent"
              size="small"
              icon={showKey ? <EyeOffRegular /> : <EyeRegular />}
              onClick={toggleShowKey}
              aria-label={showKey ? "隐藏" : "显示"}
            />
          }
        />
        <Button
          appearance="primary"
          disabled={!keyValue.trim()}
          onClick={saveKey}
        >
          保存
        </Button>
        {keyStatus === "已配置" && (
          <Badge appearance="filled" color="success">已配置</Badge>
        )}
      </div>
    </Field>
  );
}
