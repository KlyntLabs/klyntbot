import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RecallToolLogPanel } from "../RecallToolLogPanel";

vi.mock("../hooks", () => ({
  useRecallLog: () => ({
    isLoading: false,
    data: [
      {
        id: "id1",
        layer: "index",
        query: "hello",
        occurredAt: "2026-04-25T00:00:00Z",
        coverageScore: 0.42,
        latencyMs: 12,
        resultIds: ["a", "b"],
        skillUsed: null,
        sessionId: null,
        turnId: null,
        repoId: null,
        renderedTokens: null,
        metadata: null,
      },
    ],
  }),
}));

describe("RecallToolLogPanel", () => {
  it("renders rows", () => {
    render(<RecallToolLogPanel />);
    expect(screen.getByText("Recall Tool Log")).toBeInTheDocument();
    expect(screen.getByText("hello")).toBeInTheDocument();
    expect(screen.getByText(/cov=0\.42/)).toBeInTheDocument();
  });
});
