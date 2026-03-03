/**
 * 关于 Tab — 版本信息 + 链接。
 */
import { Text, Title3, Button, Link } from "@fluentui/react-components";
import { InfoRegular } from "@fluentui/react-icons";
import { createLogger } from "../../../shared/lib/logger";

const log = createLogger("AboutTab");
const APP_VERSION = "0.10.0";

export default function AboutTab() {
  const openExternal = async (url: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(url);
    } catch (e) {
      log.warn("打开外部链接失败，使用 window.open 降级:", e);
      window.open(url, "_blank");
    }
  };

  return (
    <div className="flex flex-col gap-6 py-4">
      {/* 应用信息 */}
      <div className="flex items-center gap-4">
        <InfoRegular className="text-4xl text-blue-500" />
        <div>
          <Title3>听语轩 TingYuXuan</Title3>
          <Text className="block" size={200}>
            版本 {APP_VERSION}
          </Text>
        </div>
      </div>

      {/* 链接 */}
      <div className="flex flex-col gap-2">
        <Link onClick={() => openExternal("https://github.com/user/tingyuxuan-app")}>
          GitHub 仓库
        </Link>
        <Link onClick={() => openExternal("https://github.com/user/tingyuxuan-app/issues")}>
          反馈问题
        </Link>
      </div>

      {/* 检查更新 */}
      <Button appearance="secondary" disabled>
        检查更新（即将推出）
      </Button>
    </div>
  );
}
