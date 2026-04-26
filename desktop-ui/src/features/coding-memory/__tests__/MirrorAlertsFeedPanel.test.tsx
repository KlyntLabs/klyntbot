import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MirrorAlertsFeedPanel } from "../MirrorAlertsFeedPanel";

vi.mock("../hooks", () => ({
  useMirrorAlertsFeed: () => ({
    loading: false,
    data: [
      {
        id: "alert-1",
        kind: "suggestion",
        severity: "info",
        headline: "Consider using iterator",
        payload: "{}",
        createdAt: "2026-04-25T10:00:00Z",
        dismissed: false,
      },
    ],
  }),
}));

describe("MirrorAlertsFeedPanel", () => {
  it("renders rows", () => {
    render(<MirrorAlertsFeedPanel />);
    expect(screen.getByText("Mirror Alerts")).toBeInTheDocument();
    expect(screen.getByText("Consider using iterator")).toBeInTheDocument();
    expect(screen.getByText("info")).toBeInTheDocument();
  });
});
