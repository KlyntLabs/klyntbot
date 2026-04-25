import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../hooks", () => ({
  useCostBreakdown: () => ({
    data: {
      rows: [
        {
          date: "2026-04-25",
          model: "claude-sonnet-4-6",
          source: "hooks",
          requests: 4,
          inputTokens: 250000,
          outputTokens: 80000,
          cachedTokens: 12000,
          inputCostUsd: 0.75,
          outputCostUsd: 1.2,
          totalCostUsd: 1.95,
        },
        {
          date: "2026-04-25",
          model: "deepseek-chat",
          source: "hooks",
          requests: 2,
          inputTokens: 50000,
          outputTokens: 10000,
          cachedTokens: 0,
          inputCostUsd: 0.0135,
          outputCostUsd: 0.011,
          totalCostUsd: 0.0245,
        },
      ],
      totalCostUsd: 1.9745,
      totalInputTokens: 300000,
      totalOutputTokens: 90000,
      totalCachedTokens: 12000,
      totalRequests: 6,
      bucketCount: 2,
      modelCount: 2,
      dayCount: 1,
    },
    isLoading: false,
  }),
}));

import { CostTrackerPanel } from "../CostTrackerPanel";

describe("CostTrackerPanel", () => {
  it("renders aggregate tiles + per-(date × model) rows", () => {
    render(<CostTrackerPanel />);
    expect(screen.getByText("Cost Tracker")).toBeInTheDocument();
    expect(screen.getByText("Total cost")).toBeInTheDocument();
    expect(screen.getByText("$1.97")).toBeInTheDocument();
    expect(screen.getByText("claude-sonnet-4-6")).toBeInTheDocument();
    expect(screen.getByText("deepseek-chat")).toBeInTheDocument();
    expect(screen.getByText("$1.95")).toBeInTheDocument();
  });
});
