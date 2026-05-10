import { describe, expect, it } from "vitest";
import type { ChatThread } from "@/features/chat/types";
import type { WorkspaceInfo } from "@/types";
import { groupCodingThreadsByProject } from "./groupCodingThreadsByProject";

function thread(id: string): ChatThread {
  return { sessionKey: id, title: id, updatedAt: "0", messageCount: 0 };
}

function workspace(id: string, name: string, path = `/p/${name}`): WorkspaceInfo {
  return { id, name, path, connected: true, settings: {} as WorkspaceInfo["settings"] };
}

describe("groupCodingThreadsByProject", () => {
  it("buckets threads under their workspace and preserves workspace input order", () => {
    const ws = [workspace("a", "Alpha"), workspace("b", "Beta")];
    const result = groupCodingThreadsByProject(
      [thread("t1"), thread("t2"), thread("t3")],
      new Map([
        ["t1", "b"],
        ["t2", "a"],
        ["t3", "a"],
      ]),
      ws,
    );
    expect(result.map((g) => g.workspaceId)).toEqual(["a", "b"]);
    expect(result[0].threads.map((t) => t.sessionKey)).toEqual(["t2", "t3"]);
    expect(result[1].threads.map((t) => t.sessionKey)).toEqual(["t1"]);
  });

  it("groups orphan threads (unknown workspaceId) into 'Other chats' last", () => {
    const ws = [workspace("a", "Alpha")];
    const result = groupCodingThreadsByProject(
      [thread("t1"), thread("t2")],
      new Map([["t1", "a"]]),
      ws,
    );
    expect(result).toHaveLength(2);
    expect(result[1].workspaceId).toBeNull();
    expect(result[1].name).toBe("Other chats");
    expect(result[1].threads.map((t) => t.sessionKey)).toEqual(["t2"]);
  });

  it("falls back to the path basename when a workspace has no name", () => {
    const ws: WorkspaceInfo[] = [
      {
        id: "a",
        name: "",
        path: "/Users/jayden/Projects/test-complex-session",
        connected: true,
        settings: {} as WorkspaceInfo["settings"],
      },
    ];
    const result = groupCodingThreadsByProject([thread("t1")], new Map([["t1", "a"]]), ws);
    expect(result[0].name).toBe("test-complex-session");
  });

  it("returns an empty array when no sessions and no workspaces exist", () => {
    expect(groupCodingThreadsByProject([], new Map(), [])).toEqual([]);
  });

  it("seeds a bucket for every workspace even when no threads exist yet", () => {
    const ws = [workspace("a", "Alpha"), workspace("b", "Beta")];
    const result = groupCodingThreadsByProject([], new Map(), ws);
    expect(result.map((g) => g.workspaceId)).toEqual(["a", "b"]);
    expect(result.every((g) => g.threads.length === 0)).toBe(true);
    expect(result[0].name).toBe("Alpha");
  });
});
