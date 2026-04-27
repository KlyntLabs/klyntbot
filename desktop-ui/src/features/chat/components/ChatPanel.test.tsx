// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");
vi.mock("../hooks/useChatSession", () => ({
  useChatSession: vi.fn(),
}));
vi.mock("@/features/messages/components/Markdown", () => ({
  Markdown: ({ value }: { value: string }) => <div data-testid="md">{value}</div>,
}));

import { useChatSession } from "../hooks/useChatSession";
import { ChatPanel } from "./ChatPanel";

const baseSession = {
  messages: [],
  segments: [],
  transparency: null,
  isStreaming: false,
  activeTools: [],
  error: null,
  activeInteraction: null,
  activeDelegateAgent: null,
  statusPhase: null,
  personaMessages: [],
  debateRounds: [],
  totalDebateRounds: null,
  squadMode: null,
  judgeDecisions: [],
  consensusReached: false,
  consensusSummary: null,
  input: "",
  setInput: vi.fn(),
  send: vi.fn(),
  clearInteraction: vi.fn(),
};

describe("ChatPanel", () => {
  it("renders empty state when no messages", () => {
    vi.mocked(useChatSession).mockReturnValue(baseSession);
    render(<ChatPanel sessionKey="chat:test" onThreadsChanged={() => {}} />);
    expect(screen.getByText(/start a conversation/i)).toBeTruthy();
  });

  it("renders messages when present", () => {
    vi.mocked(useChatSession).mockReturnValue({
      ...baseSession,
      messages: [
        { id: "1", role: "user", content: "hello" },
        { id: "2", role: "assistant", content: "hi there" },
      ],
    });
    render(<ChatPanel sessionKey="chat:test" onThreadsChanged={() => {}} />);
    expect(screen.getByText("hello")).toBeTruthy();
    expect(screen.getByText("hi there")).toBeTruthy();
  });
});
