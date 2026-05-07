import { describe, expect, it } from "vitest";
import type { ChatThread } from "@/features/chat/types";
import { partitionCodingThreads } from "./partitionCodingThreads";

function thread(id: string): ChatThread {
  return {
    sessionKey: id,
    title: id,
    updatedAt: new Date().toISOString(),
    messageCount: 0,
  };
}

describe("partitionCodingThreads", () => {
  it("partitions into three disjoint groups", () => {
    const sessions = [thread("a"), thread("b"), thread("c"), thread("d")];
    const running = new Set(["a", "b"]);
    const recent = new Map<string, number>([["c", Date.now()]]);

    const { running: r, recent: rc, chats } = partitionCodingThreads(sessions, running, recent);

    expect(r.map((t) => t.sessionKey)).toEqual(["a", "b"]);
    expect(rc.map((t) => t.sessionKey)).toEqual(["c"]);
    expect(chats.map((t) => t.sessionKey)).toEqual(["d"]);
  });

  it("running takes precedence over recent (same id in both)", () => {
    const sessions = [thread("a")];
    const running = new Set(["a"]);
    const recent = new Map<string, number>([["a", Date.now()]]);

    const { running: r, recent: rc, chats } = partitionCodingThreads(sessions, running, recent);

    expect(r.map((t) => t.sessionKey)).toEqual(["a"]);
    expect(rc).toEqual([]);
    expect(chats).toEqual([]);
  });

  it("preserves original order within each group", () => {
    const sessions = [thread("z"), thread("a"), thread("m")];
    const running = new Set<string>();
    const recent = new Map<string, number>();

    const { chats } = partitionCodingThreads(sessions, running, recent);
    expect(chats.map((t) => t.sessionKey)).toEqual(["z", "a", "m"]);
  });

  it("empty inputs produce empty groups", () => {
    const { running, recent, chats } = partitionCodingThreads([], new Set(), new Map());
    expect(running).toEqual([]);
    expect(recent).toEqual([]);
    expect(chats).toEqual([]);
  });
});
