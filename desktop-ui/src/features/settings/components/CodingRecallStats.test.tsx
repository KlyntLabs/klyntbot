import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { CodingRecallStats } from "./CodingRecallStats";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "coding_recall_stats") {
      return {
        totalInvocations: 42,
        meanLatencyMs: 18.5,
        topFacts: [
          { factId: "f1", subject: "logger", predicate: "uses", recallCount: 7 },
        ],
        daysWindow: 7,
      };
    }
    throw new Error(`unexpected cmd ${cmd}`);
  }),
}));

describe("CodingRecallStats", () => {
  it("renders summary + top facts", async () => {
    render(<CodingRecallStats workspaceId="ws-1" />);
    await waitFor(() => expect(screen.getByText("42")).toBeInTheDocument());
    expect(screen.getByText(/18\.5 ms/)).toBeInTheDocument();
    expect(screen.getByText(/logger\.uses/)).toBeInTheDocument();
  });
});
