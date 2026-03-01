import { describe, it, expect, vi } from "vitest";
import { screen } from "@testing-library/react";
import { renderWithProviders } from "../../test-utils";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import StatsGrid from "./StatsGrid";
import type { DashboardStats } from "../../shared/lib/types";

const MOCK_STATS: DashboardStats = {
  total_sessions: 20,
  successful_sessions: 18,
  total_duration_ms: 7_200_000, // 2 小时
  total_char_count: 5000,
  average_speed_cpm: 150,
  estimated_time_saved_ms: 3_600_000, // 1 小时
  dictionary_utilization: 0.75,
};

describe("StatsGrid", () => {
  it("渲染 5 张统计卡片", () => {
    renderWithProviders(<StatsGrid stats={MOCK_STATS} />);

    expect(screen.getByText("总录音时间")).toBeInTheDocument();
    expect(screen.getByText("总字数")).toBeInTheDocument();
    expect(screen.getByText("节省时间")).toBeInTheDocument();
    expect(screen.getByText("平均速度")).toBeInTheDocument();
    expect(screen.getByText("个性化")).toBeInTheDocument();
  });

  it("formatDuration 正确格式化小时", () => {
    renderWithProviders(<StatsGrid stats={MOCK_STATS} />);

    // 2 小时 0 分
    expect(screen.getByText("2 小时 0 分")).toBeInTheDocument();
  });

  it("formatDuration 正确格式化分钟", () => {
    const stats = { ...MOCK_STATS, total_duration_ms: 300_000 }; // 5 分钟
    renderWithProviders(<StatsGrid stats={stats} />);

    expect(screen.getByText("5 分钟")).toBeInTheDocument();
  });

  it("字数格式化包含千分位", () => {
    renderWithProviders(<StatsGrid stats={MOCK_STATS} />);

    expect(screen.getByText("5,000")).toBeInTheDocument();
  });

  it("词典利用率显示百分比", () => {
    renderWithProviders(<StatsGrid stats={MOCK_STATS} />);

    expect(screen.getByText("75%")).toBeInTheDocument();
  });
});
