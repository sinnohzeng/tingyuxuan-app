/**
 * 账户 Tab — 占位组件。
 *
 * 未来支持登录功能时替换。
 */
import { Text, Title3 } from "@fluentui/react-components";
import { PersonRegular } from "@fluentui/react-icons";

export default function AccountTab() {
  return (
    <div className="flex flex-col items-center justify-center gap-4 py-12">
      <PersonRegular className="text-5xl text-gray-300" />
      <Title3>账户</Title3>
      <Text className="text-center">
        尚未登录。听语轩目前为离线模式，所有数据存储在本地。
      </Text>
    </div>
  );
}
