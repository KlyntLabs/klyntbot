import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SessionReplayPanel } from "../SessionReplayPanel";

vi.mock("../hooks", () => ({
  useSessionReplay: () => ({
    data: [
      {
        id: "e1",
        source: "claude-code",
        sessionId: "s-1",
        kind: "userPrompt",
        occurredAt: "2026-04-23T10:00:00Z",
        payload: JSON.stringify({ v: "V1", kind: "userPrompt", text: "hi" }),
      },
    ],
  }),
  useSessionRecallOverlay: (sessionId: string) => ({
    data:
      sessionId === "s-1"
        ? [
            {
              id: "r1",
              sessionId: "s-1",
              layer: "index",
              query: "how does X work",
              repo: "local:bot",
              coverageScore: 0.42,
              durationMs: 12,
              skillUsed: null,
              createdAt: "2026-04-23T10:00:01Z",
            },
            {
              id: "r2",
              sessionId: "s-1",
              layer: "fetch",
              query: "fetch IDs",
              repo: "local:bot",
              coverageScore: 0.81,
              durationMs: 8,
              skillUsed: "query_rewriter",
              createdAt: "2026-04-23T10:00:05Z",
            },
          ]
        : [],
  }),
}));

describe("SessionReplayPanel — recall overlay", () => {
  it("renders the recall overlay rows when a session is selected", () => {
    render(<SessionReplayPanel />);

    // Open the detail panel by clicking the kind cell.
    fireEvent.click(screen.getByText("userPrompt"));

    // Overlay header + per-row layer/query are visible.
    expect(screen.getByText(/Recall on this session/i)).toBeInTheDocument();
    expect(screen.getByText("index")).toBeInTheDocument();
    expect(screen.getByText("fetch")).toBeInTheDocument();
    expect(screen.getByText("how does X work")).toBeInTheDocument();
    expect(screen.getByText("fetch IDs")).toBeInTheDocument();
  });
});
