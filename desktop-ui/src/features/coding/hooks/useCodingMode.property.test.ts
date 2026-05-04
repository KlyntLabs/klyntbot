// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { useCodingMode } from "./useCodingMode";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "chat_get_session") return { conversationType: "general" };
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

describe("K7: mode-toggle event ordering", () => {
  test("setMode is a no-op — mode does not change", async () => {
    const { result } = renderHook(() => useCodingMode("session-k7"));
    await act(async () => {
      await new Promise((r) => setTimeout(r, 1));
    });

    const initialMode = result.current.mode;
    for (let i = 0; i < 5; i++) {
      await act(async () => {
        await result.current.setMode(i % 2 === 0 ? "coding" : "general");
      });
      expect(result.current.mode).toBe(initialMode);
      expect(result.current.loading).toBe(false);
    }
  });
});
