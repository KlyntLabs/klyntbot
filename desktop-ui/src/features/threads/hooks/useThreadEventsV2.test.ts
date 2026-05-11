import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useThreadEventsV2 } from "./useThreadEventsV2";

// Tauri event mock is provided by vitest.setup.ts

describe("useThreadEventsV2", () => {
  it("listens for thread:event and filters by session_key", async () => {
    const onEvent = vi.fn();
    renderHook(() => useThreadEventsV2("session-1", onEvent));

    // Wait for listen promise to resolve
    await new Promise((r) => setTimeout(r, 10));

    // Simulate emitting an event for the wrong session
    const listeners = (window as any).__TAURI_EVENT_LISTENERS__;
    if (listeners && listeners["thread:event"]) {
      for (const cb of listeners["thread:event"]) {
        cb({ payload: { event: "content_chunk", session_key: "session-2", generation: 0, data: "hello" } });
      }
    }

    expect(onEvent).not.toHaveBeenCalled();

    // Simulate emitting an event for the right session
    if (listeners && listeners["thread:event"]) {
      for (const cb of listeners["thread:event"]) {
        cb({ payload: { event: "content_chunk", session_key: "session-1", generation: 0, data: "hello" } });
      }
    }

    expect(onEvent).toHaveBeenCalledTimes(1);
    expect(onEvent).toHaveBeenCalledWith(
      expect.objectContaining({ event: "content_chunk", session_key: "session-1" }),
    );
  });
});
