import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CliHealthPanel } from "../CliHealthPanel";

vi.mock("../hooks", () => ({
  useCliHealth: () => ({
    data: [
      {
        cli: "claude-code",
        enabled: true,
        lastEventAt: "2026-04-23T10:00:00Z",
        eventCount24h: 42,
      },
    ],
  }),
  useCodingMemoryStatus: () => ({
    data: {
      daemonAlive: true,
      bufferedEventCount: 0,
      unprocessedEventCount: 3,
      socketPath: "/x",
    },
  }),
}));

describe("CliHealthPanel", () => {
  it("renders rows", () => {
    render(<CliHealthPanel />);
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText(/alive/i)).toBeInTheDocument();
  });
});
