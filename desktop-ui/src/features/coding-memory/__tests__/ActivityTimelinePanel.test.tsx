import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../hooks", () => ({
  useActivityTimeline: () => ({
    data: [
      { date: "2026-04-20", count: 3 },
      { date: "2026-04-21", count: 7 },
      { date: "2026-04-22", count: 0 },
    ],
    isLoading: false,
  }),
}));

import { ActivityTimelinePanel } from "../ActivityTimelinePanel";

describe("ActivityTimelinePanel", () => {
  it("renders timeline cells", () => {
    render(<ActivityTimelinePanel />);
    expect(screen.getByText("Activity Timeline")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("7")).toBeInTheDocument();
  });
});
