/**
 * 介绍滑页组件 — 多屏 swipe 展示产品功能亮点。
 *
 * Props:
 *  - slides: 滑页数据
 *  - onComplete: 最后一屏点"下一步"
 *  - onSkip: 跳过按钮
 */
import { useState } from "react";
import { Card, Title3, Text, Button } from "@fluentui/react-components";
import { ArrowRightRegular } from "@fluentui/react-icons";

export interface Slide {
  icon: React.ReactNode;
  title: string;
  description: string;
}

interface IntroSlideProps {
  slides: Slide[];
  onComplete: () => void;
  onSkip: () => void;
}

export default function IntroSlide({ slides, onComplete, onSkip }: IntroSlideProps) {
  const [activeIndex, setActiveIndex] = useState(0);
  const isLast = activeIndex === slides.length - 1;
  const slide = slides[activeIndex];

  const handleNext = () => {
    if (isLast) {
      onComplete();
    } else {
      setActiveIndex((i) => i + 1);
    }
  };

  return (
    <Card className="flex flex-col items-center gap-6 p-8 max-w-md mx-auto">
      {/* 图标 */}
      <div className="text-5xl">{slide.icon}</div>

      {/* 标题 + 描述 */}
      <Title3 align="center">{slide.title}</Title3>
      <Text align="center" className="text-gray-600">{slide.description}</Text>

      {/* 步骤指示器 */}
      <div className="flex gap-2" role="tablist" aria-label="引导步骤">
        {slides.map((_, i) => (
          <div
            key={i}
            role="tab"
            aria-selected={i === activeIndex}
            aria-label={`第 ${i + 1} 步`}
            className={`w-2 h-2 rounded-full transition-colors ${
              i === activeIndex ? "bg-blue-600" : "bg-gray-300"
            }`}
          />
        ))}
      </div>

      {/* 操作按钮 */}
      <div className="flex gap-3 w-full justify-center">
        <Button appearance="subtle" onClick={onSkip}>
          跳过
        </Button>
        <Button
          appearance="primary"
          icon={isLast ? undefined : <ArrowRightRegular />}
          iconPosition="after"
          onClick={handleNext}
          aria-label={isLast ? "开始配置" : "下一步"}
        >
          {isLast ? "开始配置" : "下一步"}
        </Button>
      </div>
    </Card>
  );
}
