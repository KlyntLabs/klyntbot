import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ReforgeCycleDiffPanel } from "../ReforgeCycleDiffPanel";

vi.mock("react-diff-viewer-continued", () => ({
  default: vi.fn(({ oldValue, newValue }: { oldValue: string; newValue: string }) => (
    <div data-testid="diff-viewer">
      <pre>{oldValue}</pre>
      <pre>{newValue}</pre>
    </div>
  )),
}));

vi.mock("../hooks", () => ({
  useReforgeCycleList: () => ({
    loading: false,
    data: [{ cycleId: "cycle-a", ranAt: "2026-04-25", repos: [], artifactsWritten: 3 }],
  }),
  useReforgeCycleDiff: () => ({
    loading: false,
    data: {
      beforeBody: "before",
      afterBody: "after",
      sectionLabels: [],
    },
  }),
}));

describe("ReforgeCycleDiffPanel", () => {
  it("renders form and diff", () => {
    render(<ReforgeCycleDiffPanel />);
    expect(screen.getByText("Reforge Cycle Diff")).toBeInTheDocument();
    expect(screen.getByText("Diff")).toBeInTheDocument();
  });
});
