/**
 * API 配置向导 — 组合复用 ApiSection，引导用户完成首次配置。
 *
 * 不重新构建 API 配置 UI，而是直接嵌入已有的 ApiSection。
 * SetupWizard 只负责步骤编排和引导文案。
 */
import { Card, Title3, Text, Button } from "@fluentui/react-components";
import ApiSection from "../settings/sections/ApiSection";

interface SetupWizardProps {
  onComplete: () => void;
}

export default function SetupWizard({ onComplete }: SetupWizardProps) {
  return (
    <Card className="flex flex-col gap-6 p-8 max-w-lg mx-auto">
      <div className="flex flex-col gap-2">
        <Title3>配置 AI 服务</Title3>
        <Text className="text-gray-600">
          填入 DashScope API Key 并点击"测试连接"验证配置。
        </Text>
      </div>

      <ApiSection />

      <div className="flex justify-end pt-2">
        <Button appearance="primary" onClick={onComplete}>
          下一步
        </Button>
      </div>
    </Card>
  );
}
