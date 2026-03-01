import {
  createLightTheme,
  createDarkTheme,
  type BrandVariants,
  type Theme,
} from "@fluentui/react-components";

/**
 * 听雨轩品牌色阶 — 基于 #3b82f6 (蓝)
 * 生成方式：Fluent 2 Theme Designer 推荐的 16 级色阶
 */
const brandVariants: BrandVariants = {
  10: "#020814",
  20: "#0a1c3d",
  30: "#0e2d64",
  40: "#0f3a80",
  50: "#10489e",
  60: "#1156bc",
  70: "#1664db",
  80: "#3b82f6",
  90: "#5a96f8",
  100: "#75a8f9",
  110: "#8eb9fa",
  120: "#a5c9fb",
  130: "#bbd8fc",
  140: "#d1e6fd",
  150: "#e6f1fe",
  160: "#f5f9ff",
};

export const lightTheme: Theme = createLightTheme(brandVariants);
export const darkTheme: Theme = createDarkTheme(brandVariants);
