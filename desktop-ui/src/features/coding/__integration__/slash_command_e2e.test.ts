// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";
import { useSlashCommands } from "../hooks/useSlashCommands";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "coding_skills_list") {
      return [
        {
          name: "alpha",
          description: "Alpha skill",
          source: "user",
          source_path: "/x",
          tags: [],
          enabled: true,
        },
      ];
    }
    if (cmd === "coding_status") {
      return {
        mode: "coding",
        profile: "curated",
        sandbox: "macos",
        cost: 0,
        tokens: 0,
        active_skills: [],
      };
    }
    return null;
  }),
}));

describe("slash command e2e", () => {
  beforeEach(() => {
    chatStreamStore.clearSegments("session-e2e");
  });

  test("typing /skills list in coding mode dispatches and renders system row", async () => {
    const { result } = renderHook(() => useSlashCommands());

    let res: { kind: string; itemKind?: string; item?: { result: { name: string }[] } } | undefined;
    await act(async () => {
      res = await result.current.dispatch("/skills list", "session-e2e");
    });

    expect(res?.kind).toBe("render");
    expect(res?.itemKind).toBe("system");
    expect(res?.item?.result[0].name).toBe("alpha");

    // Simulate what Composer.tsx does on render result
    chatStreamStore.appendSystemItem("session-e2e", res.itemKind, res.item);

    const snapshot = chatStreamStore.getSnapshot("session-e2e");
    expect(snapshot.segments.length).toBe(1);
    expect((snapshot.segments[0] as { type: string }).type).toBe("system");
  });

  test("/plan refactor is agent-routed and transforms text", async () => {
    const { result } = renderHook(() => useSlashCommands());

    let res: { kind: string; text?: string } | undefined;
    await act(async () => {
      res = await result.current.dispatch("/plan refactor parser", "session-e2e");
    });

    expect(res.kind).toBe("passthrough");
    expect(res.text).toContain("[system: enter plan mode]");
  });

  test("unknown command falls through as passthrough with original text", async () => {
    const { result } = renderHook(() => useSlashCommands());

    let res: { kind: string; text?: string } | undefined;
    await act(async () => {
      res = await result.current.dispatch("/unknown xyz", "session-e2e");
    });

    expect(res.kind).toBe("passthrough");
    expect(res.text).toBe("/unknown xyz");
  });
});
