// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useChatThreads } from "./useChatThreads";

const invokeMock = vi.mocked(invoke);
const listeners = new Map<string, (event: { payload: unknown }) => void>();

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");

beforeEach(() => {
  invokeMock.mockReset();
  vi.mocked(listen)
    .mockClear()
    .mockImplementation(async (event: string, cb: (e: { payload: unknown }) => void) => {
      listeners.set(event, cb);
      return () => listeners.delete(event);
    });
  listeners.clear();
});

afterEach(() => {
  listeners.clear();
});

describe("useChatThreads", () => {
  it("fetches threads on mount", async () => {
    invokeMock.mockResolvedValueOnce([
      { sessionKey: "chat:1", title: "First", messageCount: 1, updatedAt: "2026-04-26" },
    ]);
    const { result } = renderHook(() => useChatThreads());
    await waitFor(() => expect(result.current.threads).toHaveLength(1));
    expect(invokeMock).toHaveBeenCalledWith("chat_threads");
  });

  it("refetches on chat:thread_created event", async () => {
    invokeMock.mockResolvedValueOnce([]);
    const { result } = renderHook(() => useChatThreads());
    await waitFor(() => expect(result.current.threads).toEqual([]));

    invokeMock.mockResolvedValueOnce([
      { sessionKey: "chat:2", title: "New", messageCount: 1, updatedAt: "2026-04-26" },
    ]);

    await act(async () => {
      const cb = listeners.get("chat:thread_created");
      cb?.({ payload: {} });
    });

    await waitFor(() => expect(result.current.threads).toHaveLength(1));
  });
});
