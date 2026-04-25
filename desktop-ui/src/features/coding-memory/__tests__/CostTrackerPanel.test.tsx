import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../hooks", () => ({
  useCostRollup: () => ({
    data: [
      { period: "2026-W16", cost_usd: 0.42 },
      { period: "2026-W17", cost_usd: 1.20 },
    ],
    isLoading: false,
  }),
}));

import { CostTrackerPanel } from "../CostTrackerPanel";

describe("CostTrackerPanel", () => {
  it("renders cost bars", () => {
    render(<CostTrackerPanel />);
    expect(screen.getByText("Cost Tracker")).toBeInTheDocument();
    expect(screen.getByText("2026-W16")).toBeInTheDocument();
    expect(screen.getByText("$0.42")).toBeInTheDocument();
    expect(screen.getByText("2026-W17")).toBeInTheDocument();
    expect(screen.getByText("$1.20")).toBeInTheDocument();
  });
});
