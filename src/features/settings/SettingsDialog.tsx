/**
 * 设置弹窗 — Fluent 2 Dialog + 垂直 Tab 导航。
 *
 * 从 uiStore 读取 open 状态，关闭时自动保存配置。
 * 左侧 TabList（垂直），右侧渲染当前 tab 组件。
 */
import { lazy, Suspense } from "react";
import {
  Dialog,
  DialogSurface,
  DialogBody,
  DialogTitle,
  DialogContent,
  TabList,
  Tab,
  Spinner,
  Button,
} from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import { useUIStore, type SettingsTab } from "../../shared/stores/uiStore";
import { useConfig } from "./hooks/useConfig";
import AccountTab from "./tabs/AccountTab";
import AboutTab from "./tabs/AboutTab";
import PersonalizationTab from "./tabs/PersonalizationTab";

const SettingsTab = lazy(() => import("./tabs/SettingsTab"));

const TAB_LABELS: Record<SettingsTab, string> = {
  account: "账户",
  settings: "设置",
  personalization: "个性化",
  about: "关于",
};

export default function SettingsDialog() {
  const settingsOpen = useUIStore((s) => s.settingsOpen);
  const settingsTab = useUIStore((s) => s.settingsTab);
  const openSettings = useUIStore((s) => s.openSettings);
  const closeSettings = useUIStore((s) => s.closeSettings);
  const { config, isLoading, updateConfig, saveConfig } = useConfig();

  const handleClose = async () => {
    await saveConfig();
    closeSettings();
  };

  const handleTabSelect = (_: unknown, data: { value: unknown }) => {
    openSettings(data.value as SettingsTab);
  };

  return (
    <Dialog open={settingsOpen} onOpenChange={(_, data) => { if (!data.open) handleClose(); }}>
      <DialogSurface style={{ width: 720, maxWidth: "90vw", height: 540, maxHeight: "85vh" }}>
        <DialogBody className="flex flex-col h-full overflow-hidden">
          <DialogTitle
            action={
              <Button
                appearance="subtle"
                icon={<DismissRegular />}
                aria-label="关闭"
                onClick={handleClose}
              />
            }
          >
            设置
          </DialogTitle>

          <DialogContent className="flex flex-1 gap-4 overflow-hidden p-0 mt-2">
            {/* 左侧 Tab 导航 */}
            <TabList
              vertical
              selectedValue={settingsTab}
              onTabSelect={handleTabSelect}
              className="shrink-0 w-28 border-r border-gray-200 dark:border-gray-800 pr-2"
            >
              {(Object.keys(TAB_LABELS) as SettingsTab[]).map((key) => (
                <Tab key={key} value={key}>
                  {TAB_LABELS[key]}
                </Tab>
              ))}
            </TabList>

            {/* 右侧内容区 */}
            <div className="flex-1 overflow-y-auto pr-2">
              {isLoading ? (
                <Spinner size="medium" label="加载配置中…" />
              ) : (
                <Suspense fallback={<Spinner size="small" />}>
                  <TabContent
                    tab={settingsTab}
                    config={config}
                    updateConfig={updateConfig}
                  />
                </Suspense>
              )}
            </div>
          </DialogContent>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

/** 根据当前 tab 渲染对应组件 */
function TabContent({
  tab,
  config,
  updateConfig,
}: {
  tab: SettingsTab;
  config: ReturnType<typeof useConfig>["config"];
  updateConfig: ReturnType<typeof useConfig>["updateConfig"];
}) {
  switch (tab) {
    case "account":
      return <AccountTab />;
    case "settings":
      return config ? (
        <SettingsTab config={config} updateConfig={updateConfig} />
      ) : null;
    case "personalization":
      return <PersonalizationTab />;
    case "about":
      return <AboutTab />;
  }
}
