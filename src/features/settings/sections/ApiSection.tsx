/**
 * API 配置 section — DashScope 锁定模式。
 *
 * 只需填入 API Key + 测试连接，provider/model/url 使用默认值。
 */
import { Text, Badge } from "@fluentui/react-components";
import { InfoRegular } from "@fluentui/react-icons";
import ApiKeyField from "../components/ApiKeyField";
import ConnectionTestButton from "../components/ConnectionTestButton";

export default function ApiSection() {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <Text weight="semibold">语音识别服务</Text>
        <div className="flex items-center gap-2">
          <Badge appearance="outline" color="informative" icon={<InfoRegular />}>
            阿里云 DashScope
          </Badge>
          <Text size={200} className="text-gray-500">qwen3-omni-flash</Text>
        </div>
      </div>

      <ApiKeyField service="llm" label="DashScope API Key" />
      <ConnectionTestButton command="test_llm_connection" label="测试连接" />
    </div>
  );
}
