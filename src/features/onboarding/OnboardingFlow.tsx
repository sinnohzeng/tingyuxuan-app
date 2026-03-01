/**
 * 引导流程 — Intro 滑页 → API 配置 → 权限检查 → 进入首页。
 *
 * Windows/Linux 自动跳过权限步骤。
 * 完成后在 localStorage 标记 onboarding_complete，防止重复进入。
 */
import { useState, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import {
  FluentProvider,
  ProgressBar,
  Text,
} from "@fluentui/react-components";
import {
  MicRegular,
  TextEditStyleRegular,
  KeyboardRegular,
} from "@fluentui/react-icons";
import { useSystemTheme } from "../../shared/hooks/useSystemTheme";
import { useConfig } from "../settings/hooks/useConfig";
import IntroSlide, { type Slide } from "./IntroSlide";
import SetupWizard from "./SetupWizard";
import PermissionGuide from "./PermissionGuide";

type Step = "intro" | "setup" | "permissions";

const INTRO_SLIDES: Slide[] = [
  {
    icon: <MicRegular className="text-5xl text-blue-600" />,
    title: "语音输入，解放双手",
    description: "按下快捷键开始录音，听语轩将语音实时转为文字，注入到任何应用中。",
  },
  {
    icon: <TextEditStyleRegular className="text-5xl text-green-600" />,
    title: "AI 润色，一步到位",
    description: "多模态大模型直接处理音频，智能纠错、调整语气、适配场景。",
  },
  {
    icon: <KeyboardRegular className="text-5xl text-purple-600" />,
    title: "全局快捷键，随时待命",
    description: "自定义快捷键，无论你在哪个应用，一键唤起听语轩。",
  },
];

const STEP_LABELS: Record<Step, string> = {
  intro: "欢迎",
  setup: "配置",
  permissions: "权限",
};
const STEPS: Step[] = ["intro", "setup", "permissions"];

export default function OnboardingFlow() {
  const theme = useSystemTheme();
  const navigate = useNavigate();
  const { config, updateConfig, saveConfig } = useConfig();
  const [step, setStep] = useState<Step>("intro");

  const stepIndex = STEPS.indexOf(step);
  const progress = (stepIndex + 1) / STEPS.length;

  const markComplete = useCallback(() => {
    localStorage.setItem("onboarding_complete", "1");
    saveConfig();
    navigate("/main", { replace: true });
  }, [navigate, saveConfig]);

  const handleSkip = useCallback(() => {
    markComplete();
  }, [markComplete]);

  const stepContent = useMemo(() => {
    switch (step) {
      case "intro":
        return (
          <IntroSlide
            slides={INTRO_SLIDES}
            onComplete={() => setStep("setup")}
            onSkip={handleSkip}
          />
        );
      case "setup":
        return config ? (
          <SetupWizard
            config={config}
            updateConfig={updateConfig}
            onComplete={() => setStep("permissions")}
          />
        ) : null;
      case "permissions":
        return <PermissionGuide onComplete={markComplete} />;
    }
  }, [step, config, updateConfig, handleSkip, markComplete]);

  return (
    <FluentProvider theme={theme} className="flex flex-col h-screen">
      {/* 顶部进度 */}
      <div className="flex flex-col gap-1 px-8 pt-6">
        <div className="flex justify-between">
          <Text weight="semibold">{STEP_LABELS[step]}</Text>
          <Text size={200} className="text-gray-500">
            {stepIndex + 1} / {STEPS.length}
          </Text>
        </div>
        <ProgressBar value={progress} />
      </div>

      {/* 内容区 */}
      <div className="flex-1 flex items-center justify-center p-8">
        {stepContent}
      </div>
    </FluentProvider>
  );
}
