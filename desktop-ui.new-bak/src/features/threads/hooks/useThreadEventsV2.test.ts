import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { listen } from "@tauri-apps/api/event";
import { useThreadEventsV2 } from "./useThreadEventsV2";

describe("useThreadEventsV2", () => {
  beforeEach(() => {
    vi.mocked(listen).mockReset();
  });

  it("listens for thread:event and filters by session_key", async () => {
    let handler: ((e: { payload: Record<string, unknown> }) => void) | undefined;
    vi.mocked(listen).mockImplementation((_event, h) => {
      handler = h as (e: { payload: Record<string, unknown> }) => void;
      return Promise.resolve(() => {});
    });

    const onEvent = vi.fn();
    renderHook(() => useThreadEventsV2("session-1", onEvent));

    await waitFor(() => expect(listen).toHaveBeenCalled());

    // Simulate emitting an event for the wrong session
    handler?.({
      payload: { event: "content_chunk", session_key: "session-2", generation: 0, data: "hello" },
    });
    expect(onEvent).not.toHaveBeenCalled();

    // Simulate emitting an event for the right session
    handler?.({
      payload: { event: "content_chunk", session_key: "session-1", generation: 0, data: "hello" },
    });
    expect(onEvent).toHaveBeenCalledTimes(1);
    expect(onEvent).toHaveBeenCalledWith(
      expect.objectContaining({ event: "content_chunk", session_key: "session-1" }),
    );
  });
});
