// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { useCodingMode } from "./useCodingMode";

let invokeCalls: Array<[string, unknown]> = [];
vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string, args: unknown) => {
    invokeCalls.push([cmd, args]);
    if (cmd === "chat_get_session") return { conversationType: "general" };
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_e: string, _cb: unknown) => () => {}),
}));

describe("useCodingMode", () => {
  beforeEach(() => {
    invokeCalls = [];
  });

  test("initial fetch returns general mode", async () => {
    const { result } = renderHook(() => useCodingMode("session-1"));
    await act(async () => {
      await new Promise((r) => setTimeout(r, 1));
    });
    expect(result.current.mode).toBe("general");
  });

  test("setMode is a no-op (SessionMode is immutable)", async () => {
    const { result } = renderHook(() => useCodingMode("session-1"));
    await act(async () => {
      await new Promise((r) => setTimeout(r, 1));
    });
    const initialMode = result.current.mode;
    await act(async () => {
      await result.current.setMode("coding");
    });
    expect(result.current.mode).toBe(initialMode);
    const setCall = invokeCalls.find(([c]) => c === "chat_set_mode");
    expect(setCall).toBeFalsy();
  });
});
