import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PatternEffectivenessPanel } from "../PatternEffectivenessPanel";

vi.mock("../hooks", () => ({
  useEffectivenessTrends: () => ({
    loading: false,
    data: {
      patternId: "pat-1",
      patternName: "Extract Method",
      buckets: [
        { at: "2026-04-20", score: 0.72 },
        { at: "2026-04-21", score: 0.81 },
      ],
    },
  }),
}));

describe("PatternEffectivenessPanel", () => {
  it("renders buckets", () => {
    render(<PatternEffectivenessPanel />);
    expect(screen.getByText("Pattern Effectiveness")).toBeInTheDocument();
    expect(screen.getByText("Extract Method")).toBeInTheDocument();
    expect(screen.getByText("0.72")).toBeInTheDocument();
  });
});
