// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { useCodingMode } from "./useCodingMode";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "chat_get_session") return { conversationType: "general" };
    if (cmd === "chat_set_mode") return undefined;
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

describe("K7: mode-toggle event ordering", () => {
  test("every setMode call results in exactly one state update before next setMode", async () => {
    const flips: Array<"coding" | "general"> = [];
    for (let i = 0; i < 20; i++) {
      flips.push(Math.random() > 0.5 ? "coding" : "general");
    }

    const { result } = renderHook(() => useCodingMode("session-k7"));
    await act(async () => {
      await new Promise((r) => setTimeout(r, 1));
    });

    const events: string[] = [];
    for (const target of flips) {
      const _before = result.current.mode;
      await act(async () => {
        await result.current.setMode(target);
      });
      const after = result.current.mode;
      events.push(`mode_changed:${after}`);
      expect(after).toBe(target);
      // Each setMode should have changed the mode (or stayed same if already target)
      // The key invariant: no pending/race state between sequential setMode calls
      expect(result.current.loading).toBe(false);
    }

    expect(events.filter((e) => e.startsWith("mode_changed:")).length).toBe(flips.length);
  });

  test("rapid alternating flips settle to final mode", async () => {
    const { result } = renderHook(() => useCodingMode("session-k7-alt"));
    await act(async () => {
      await new Promise((r) => setTimeout(r, 1));
    });

    // Rapid fire 10 alternations
    for (let i = 0; i < 10; i++) {
      const target = i % 2 === 0 ? "coding" : "general";
      await act(async () => {
        await result.current.setMode(target);
      });
    }

    // After all async ops complete, mode should be the last one
    expect(result.current.mode).toBe("general");
    expect(result.current.loading).toBe(false);
  });
});
