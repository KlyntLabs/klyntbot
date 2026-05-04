// @vitest-environment jsdom

import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@/api/client", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@/features/chat/store/chatStreamStore", () => ({
  chatStreamStore: { upsertApproval: vi.fn(), resolveApproval: vi.fn() },
}));

import { listen } from "@tauri-apps/api/event";
import { invoke } from "@/api/client";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";
import { useApprovalQueue } from "./useApprovalQueue";

describe("useApprovalQueue", () => {
  beforeEach(() => vi.clearAllMocks());

  it("subscribes to approval events", async () => {
    vi.mocked(listen).mockImplementation(() => Promise.resolve(() => {}));
    renderHook(() => useApprovalQueue("session-1"));
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));
  });

  it("respond invokes chat_respond_approval", async () => {
    vi.mocked(listen).mockImplementation(() => Promise.resolve(() => {}));
    const { result } = renderHook(() => useApprovalQueue("session-1"));
    await act(async () => {
      await result.current.respond("r1", { kind: "allow_once" });
    });
    expect(invoke).toHaveBeenCalledWith("chat_respond_approval", {
      sessionKey: "session-1",
      requestId: "r1",
      decision: { kind: "allow_once" },
    });
  });

  it("upserts approval on agent:approval_requested when requires_user_input is true", async () => {
    const listeners: Record<string, (event: { payload: unknown }) => void> = {};
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners[event as string] = handler as (e: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });
    renderHook(() => useApprovalQueue("session-1"));
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));

    listeners["agent:approval_requested"]({
      payload: {
        request_id: "r1",
        tool: "bash",
        args: { command: "ls" },
        cwd: "/repo",
        sandbox_summary: "Seatbelt cwd-only",
        layer: "layer2_starlark",
        layer_reason: "no rule matched",
        requires_user_input: true,
      },
    });

    expect(chatStreamStore.upsertApproval).toHaveBeenCalledWith(
      "session-1",
      expect.objectContaining({ kind: "approval", requestId: "r1", status: "pending" }),
    );
  });

  it("resolves approval on agent:approval_resolved", async () => {
    const listeners: Record<string, (event: { payload: unknown }) => void> = {};
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners[event as string] = handler as (e: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });
    renderHook(() => useApprovalQueue("session-1"));
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));

    listeners["agent:approval_resolved"]({
      payload: {
        request_id: "r1",
        decided_by: "user",
        decision_reason: "",
      },
    });

    expect(chatStreamStore.resolveApproval).toHaveBeenCalledWith(
      "session-1",
      "r1",
      "approved-once",
      "user",
    );
  });
});
