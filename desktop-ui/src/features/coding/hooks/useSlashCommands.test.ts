// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { useSlashCommands } from "./useSlashCommands";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string, _args: unknown) => {
    if (cmd === "coding_skills_list")
      return [
        {
          name: "alpha",
          description: "Alpha",
          source: "user",
          source_path: "/x",
          tags: [],
          enabled: true,
        },
      ];
    if (cmd === "coding_status")
      return {
        mode: "coding",
        profile: "curated",
        sandbox: "macos",
        cost: 0,
        tokens: 0,
        active_skills: [],
      };
    return null;
  }),
}));

describe("useSlashCommands", () => {
  beforeEach(() => vi.clearAllMocks());

  test("classify routes /skills list → direct", () => {
    const { result } = renderHook(() => useSlashCommands());
    expect(result.current.classify("/skills list")).toBe("direct");
  });

  test("dispatch direct invokes Tauri command", async () => {
    const { result } = renderHook(() => useSlashCommands());
    let res: any;
    await act(async () => {
      res = await result.current.dispatch("/skills list", "session-1");
    });
    expect(res.kind).toBe("render");
    expect(res.itemKind).toBe("system");
  });

  test("dispatch agent-routed returns passthrough", async () => {
    const { result } = renderHook(() => useSlashCommands());
    let res: any;
    await act(async () => {
      res = await result.current.dispatch("/plan refactor", "session-1");
    });
    expect(res.kind).toBe("passthrough");
    expect(res.text).toContain("[system: enter plan mode]");
  });

  test("dispatch unknown returns passthrough with original text", async () => {
    const { result } = renderHook(() => useSlashCommands());
    let res: any;
    await act(async () => {
      res = await result.current.dispatch("/foobar abc", "session-1");
    });
    expect(res.kind).toBe("passthrough");
    expect(res.text).toBe("/foobar abc");
  });

  test("catalog returns flatCatalog", () => {
    const { result } = renderHook(() => useSlashCommands());
    const cat = result.current.catalog();
    expect(cat.length).toBeGreaterThan(8);
  });
});
