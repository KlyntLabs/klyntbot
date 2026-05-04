/** @vitest-environment jsdom */
import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "coding_thread_list") {
      return [
        { id: "coding:abc", title: "Fix bug", updatedAt: 1, workspaceId: "ws1" },
      ];
    }
    return [];
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

afterEach(() => vi.restoreAllMocks());

describe("useCodingSessions", () => {
  it("returns coding sessions reshaped to ChatThread", async () => {
    const { useCodingSessions } = await import("./useCodingSessions");
    const { result } = renderHook(() => useCodingSessions());
    await waitFor(() => expect(result.current.threads.length).toBe(1));
    expect(result.current.threads[0].sessionKey).toBe("coding:abc");
    expect(result.current.threads[0].title).toBe("Fix bug");
  });
});
