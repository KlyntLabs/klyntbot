import { describe, expect, it } from "vitest";
import type { ConversationItem } from "@/types";
import { groupBursts, type BurstGroup } from "./groupBursts";

function tool(
  id: string,
  toolType: string,
  title: string,
  status: string = "completed",
  detail: string = "",
): Extract<ConversationItem, { kind: "tool" }> {
  return { id, kind: "tool", toolType, title, detail, status, output: "" };
}

function read(id: string, path: string) {
  return tool(id, "fileChange", "File changes", "completed", path);
}

describe("groupBursts", () => {
  it("returns input unchanged when no group of 3+ exists", () => {
    const items: ConversationItem[] = [read("a", "x.ts"), read("b", "y.ts")];
    expect(groupBursts(items)).toEqual(items);
  });

  it("collapses 3 consecutive same-family same-name reads", () => {
    const items: ConversationItem[] = [
      read("a", "x.ts"),
      read("b", "y.ts"),
      read("c", "z.ts"),
    ];
    const out = groupBursts(items);
    expect(out).toHaveLength(1);
    const burst = out[0] as BurstGroup;
    expect(burst.kind).toBe("burst");
    expect(burst.items).toHaveLength(3);
    expect(burst.family).toBe("filesystem");
    expect(burst.name).toBe("Edit");
  });

  it("breaks a group when a failed tool appears in the middle", () => {
    const items: ConversationItem[] = [
      read("a", "x.ts"),
      read("b", "y.ts"),
      tool("c", "fileChange", "File changes", "failed", "z.ts"),
      read("d", "p.ts"),
      read("e", "q.ts"),
      read("f", "r.ts"),
    ];
    const out = groupBursts(items);
    expect(out.map((x) => ("kind" in x && x.kind === "burst" ? "burst" : x.id))).toEqual([
      "a",
      "b",
      "c",
      "burst",
    ]);
  });

  it("does not group across different families", () => {
    const items: ConversationItem[] = [
      read("a", "x.ts"),
      read("b", "y.ts"),
      tool("c", "commandExecution", "Command: ls"),
      read("d", "z.ts"),
    ];
    const out = groupBursts(items);
    expect(out).toHaveLength(4);
  });

  it("does not group different names within same family", () => {
    const reads = [read("a", "x.ts"), read("b", "y.ts")];
    const writes: ConversationItem[] = [
      {
        id: "c",
        kind: "tool",
        toolType: "fileChange",
        title: "File changes",
        detail: "",
        status: "completed",
        output: "",
        changes: [{ path: "n.ts", kind: "add", diff: "@@ +1 @@\n+x" }],
      },
    ];
    expect(groupBursts([...reads, ...writes])).toHaveLength(3);
  });
});
