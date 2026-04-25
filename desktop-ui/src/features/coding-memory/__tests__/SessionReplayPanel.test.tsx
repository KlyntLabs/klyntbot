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
}));

describe("SessionReplayPanel", () => {
  it("renders rows and opens detail", () => {
    render(<SessionReplayPanel />);
    expect(screen.getByText("userPrompt")).toBeInTheDocument();
    fireEvent.click(screen.getByText("userPrompt"));
    expect(screen.getByText(/Event detail/i)).toBeInTheDocument();
  });
});
