/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useKlyntbotSurfaceProps } from "./useKlyntbotSurfaceProps";

vi.mock("./useChatSession", () => ({
  useChatSession: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", async () => {
  const { mockTauriCore } = await import("@/test/mockTauri");
  return mockTauriCore();
});

vi.mock("@tauri-apps/api/event", async () => {
  const { mockTauriEvent } = await import("@/test/mockTauri");
  return mockTauriEvent();
});

import { useChatSession } from "./useChatSession";
import { invoke } from "@tauri-apps/api/core";

const mockUseChatSession = vi.mocked(useChatSession);
const mockInvoke = vi.mocked(invoke);

type Session = ReturnType<typeof useChatSession>;

const baseSession: Session = {
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

describe("useKlyntbotSurfaceProps", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseChatSession.mockReturnValue(baseSession);
    mockInvoke.mockResolvedValue(undefined);
  });

  it("returns null when sessionKey is null", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps(null));
    expect(result.current).toBeNull();
  });

  it("returns an override object when sessionKey is provided", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current).not.toBeNull();
    expect(result.current?.messagesProps).toBeDefined();
    expect(result.current?.composerProps).toBeDefined();
  });

  it("maps persisted user/assistant messages into kind: 'message' rows", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [
        { id: "m1", role: "user", content: "hello" },
        { id: "m2", role: "assistant", content: "hi there" },
      ],
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "m1", kind: "message", role: "user", text: "hello" },
      { id: "m2", kind: "message", role: "assistant", text: "hi there" },
    ]);
  });

  it("coerces role: 'interaction' messages to assistant rows", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [{ id: "m3", role: "interaction", content: "Q: which file?" }],
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "m3", kind: "message", role: "assistant", text: "Q: which file?" },
    ]);
  });

  it("coalesces streaming text segments into a single trailing assistant message", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [{ id: "m1", role: "user", content: "ping" }],
      segments: [
        { type: "text", content: "po" },
        { type: "text", content: "ng" },
      ],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "m1", kind: "message", role: "user", text: "ping" },
      { id: "stream-assistant-0", kind: "message", role: "assistant", text: "pong" },
    ]);
  });

  it("does not append a streaming row when there are no text segments", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      messages: [{ id: "m1", role: "user", content: "ping" }],
      segments: [],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toHaveLength(1);
  });

  it("maps tool segments to kind: 'tool' rows interleaved with text", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      segments: [
        { type: "text", content: "checking..." },
        {
          type: "tool",
          name: "search",
          action: "find foo",
          success: true,
          durationMs: 120,
          result: "no matches",
        },
        { type: "text", content: "done" },
      ],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items).toEqual([
      { id: "stream-assistant-0", kind: "message", role: "assistant", text: "checking..." },
      {
        id: "stream-tool-1",
        kind: "tool",
        toolType: "search",
        title: "search",
        detail: "find foo",
        output: "no matches",
        durationMs: 120,
        status: "completed",
      },
      { id: "stream-assistant-2", kind: "message", role: "assistant", text: "done" },
    ]);
  });

  it("emits failed tool rows with status: 'failed'", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      segments: [
        {
          type: "tool",
          name: "search",
          success: false,
          durationMs: 50,
          result: "error",
        },
      ],
      isStreaming: true,
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.items[0]).toMatchObject({
      kind: "tool",
      status: "failed",
    });
  });

  it("maps activeInteraction into userInputRequests with shape conversion", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      activeInteraction: {
        requestId: "req-7",
        request: {
          title: "Pick a target",
          questions: [
            {
              id: "q1",
              title: "Which file?",
              text: "Choose the file to operate on.",
              answer_type: { type: "free_text" },
            },
          ],
        },
      },
    });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.userInputRequests).toEqual([
      {
        workspace_id: "",
        request_id: "req-7",
        params: {
          thread_id: "session-1",
          turn_id: "",
          item_id: "req-7",
          questions: [
            {
              id: "q1",
              header: "Which file?",
              question: "Choose the file to operate on.",
            },
          ],
        },
      },
    ]);
  });

  it("returns empty userInputRequests when no active interaction", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.messagesProps.userInputRequests).toEqual([]);
  });

  it("invokes chat_respond_interaction when onUserInputSubmit fires", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      activeInteraction: {
        requestId: "req-99",
        request: {
          title: "t",
          questions: [
            { id: "q1", title: "T", text: "?", answer_type: { type: "free_text" } },
          ],
        },
      },
    });

    const { result } = renderHook(() => useKlyntbotSurfaceProps("sk"));
    const req = result.current!.messagesProps.userInputRequests![0];
    result.current!.messagesProps.onUserInputSubmit!(req, {
      answers: { q1: { answers: ["yes"] } },
    });

    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_interaction", {
      sessionKey: "sk",
      requestId: "req-99",
      response: {
        Completed: [
          { question_id: "q1", value: { type: "text", content: "yes" } },
        ],
      },
    });
  });

  it("maps yes_no answers correctly in onUserInputSubmit", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      activeInteraction: {
        requestId: "req-y",
        request: {
          title: "t",
          questions: [
            { id: "q1", title: "Sure?", text: "?", answer_type: { type: "yes_no" } },
          ],
        },
      },
    });

    const { result } = renderHook(() => useKlyntbotSurfaceProps("sk"));
    const req = result.current!.messagesProps.userInputRequests![0];
    result.current!.messagesProps.onUserInputSubmit!(req, {
      answers: { q1: { answers: ["true"] } },
    });

    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_interaction", {
      sessionKey: "sk",
      requestId: "req-y",
      response: {
        Completed: [
          { question_id: "q1", value: { type: "yes_no", answer: true } },
        ],
      },
    });
  });

  it("maps single_select answers correctly in onUserInputSubmit", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      activeInteraction: {
        requestId: "req-s",
        request: {
          title: "t",
          questions: [
            {
              id: "q1",
              title: "Pick",
              text: "?",
              answer_type: { type: "single_select", options: [{ value: "a", label: "A" }] },
            },
          ],
        },
      },
    });

    const { result } = renderHook(() => useKlyntbotSurfaceProps("sk"));
    const req = result.current!.messagesProps.userInputRequests![0];
    result.current!.messagesProps.onUserInputSubmit!(req, {
      answers: { q1: { answers: ["a"] } },
    });

    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_interaction", {
      sessionKey: "sk",
      requestId: "req-s",
      response: {
        Completed: [
          { question_id: "q1", value: { type: "selected", value: "a" } },
        ],
      },
    });
  });

  it("maps multi_select answers correctly in onUserInputSubmit", () => {
    mockUseChatSession.mockReturnValue({
      ...baseSession,
      activeInteraction: {
        requestId: "req-m",
        request: {
          title: "t",
          questions: [
            {
              id: "q1",
              title: "Pick",
              text: "?",
              answer_type: { type: "multi_select", options: [{ value: "a", label: "A" }, { value: "b", label: "B" }] },
            },
          ],
        },
      },
    });

    const { result } = renderHook(() => useKlyntbotSurfaceProps("sk"));
    const req = result.current!.messagesProps.userInputRequests![0];
    result.current!.messagesProps.onUserInputSubmit!(req, {
      answers: { q1: { answers: ["a", "b"] } },
    });

    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_interaction", {
      sessionKey: "sk",
      requestId: "req-m",
      response: {
        Completed: [
          { question_id: "q1", value: { type: "multi_selected", values: ["a", "b"] } },
        ],
      },
    });
  });

  it("sets processingStartedAt on the false→true streaming edge", () => {
    const beforeStart = Date.now() - 1;
    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: false });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    expect(result.current?.messagesProps.processingStartedAt).toBeNull();

    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: true });
    rerender();
    const ts = result.current?.messagesProps.processingStartedAt;
    expect(typeof ts).toBe("number");
    expect(ts).toBeGreaterThanOrEqual(beforeStart);
  });

  it("clears processingStartedAt on streaming true→false edge", () => {
    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: true });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    expect(typeof result.current?.messagesProps.processingStartedAt).toBe("number");

    mockUseChatSession.mockReturnValue({ ...baseSession, isStreaming: false });
    rerender();
    expect(result.current?.messagesProps.processingStartedAt).toBeNull();
  });

  it("calls chat.send when composer onSend fires", () => {
    const sendSpy = vi.fn();
    mockUseChatSession.mockReturnValue({ ...baseSession, send: sendSpy });
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    result.current!.composerProps.onSend("hello", []);
    expect(sendSpy).toHaveBeenCalledTimes(1);
  });

  it("supplies a single-item models[] so the model pill renders", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.composerProps.models).toEqual([
      { id: "klyntbot", displayName: "klyntbot", model: "klyntbot" },
    ]);
    expect(result.current?.composerProps.selectedModelId).toBe("klyntbot");
  });

  it("supplies a default collaboration mode so the pill stays visible", () => {
    const { result } = renderHook(() => useKlyntbotSurfaceProps("session-1"));
    expect(result.current?.composerProps.collaborationModes).toEqual([
      { id: "default", label: "klyntbot" },
    ]);
    expect(result.current?.composerProps.selectedCollaborationModeId).toBe("default");
  });

  it("exposes chat.error and clears it on onDismissError", () => {
    mockUseChatSession.mockReturnValue({ ...baseSession, error: "oops" });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    expect(result.current?.error).toBe("oops");

    act(() => {
      result.current!.onDismissError();
    });
    rerender();
    expect(result.current?.error).toBeNull();
  });

  it("re-shows error when a different error string arrives after dismissal", () => {
    mockUseChatSession.mockReturnValue({ ...baseSession, error: "first" });
    const { result, rerender } = renderHook(() =>
      useKlyntbotSurfaceProps("session-1"),
    );
    act(() => {
      result.current!.onDismissError();
    });
    rerender();
    expect(result.current?.error).toBeNull();

    mockUseChatSession.mockReturnValue({ ...baseSession, error: "second" });
    rerender();
    expect(result.current?.error).toBe("second");
  });
});
