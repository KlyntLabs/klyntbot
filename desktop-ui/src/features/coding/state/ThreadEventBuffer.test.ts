/** @vitest-environment jsdom */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

afterEach(() => {
  vi.useRealTimers();
});

describe("ThreadEventBuffer — initial state", () => {
  it("starts with empty running and recently completed", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
    expect(mod.getRunningIds().size).toBe(0);
    expect(mod.getRecentlyCompleted().size).toBe(0);
  });
});

describe("ThreadEventBuffer — turn lifecycle", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("turn_started adds id to running set", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      model: "x",
      started_at: 0,
    });
    expect(mod.getRunningIds().has("coding:t1")).toBe(true);
  });

  it("turn_completed removes id from running and adds to recently completed", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      model: "x",
      started_at: 0,
    });
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 100,
      duration_ms: 100,
    });
    expect(mod.getRunningIds().has("coding:t1")).toBe(false);
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
  });

  it("turn_started after turn_completed clears recently-completed entry", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 100,
      duration_ms: 100,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:t1",
      turn_id: "turn-2",
      model: "x",
      started_at: 200,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(false);
    expect(mod.getRunningIds().has("coding:t1")).toBe(true);
  });
});

describe("ThreadEventBuffer — recently completed lifecycle", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("markThreadOpened removes from recently completed", async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 0,
      duration_ms: 0,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
    mod.markThreadOpened("coding:t1");
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(false);
  });

  it("30-minute timer auto-removes from recently completed", async () => {
    vi.useFakeTimers();
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.applyEvent({
      kind: "turn_completed",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      finish_reason: "stop",
      completed_at: 0,
      duration_ms: 0,
    });
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(true);
    vi.advanceTimersByTime(30 * 60 * 1000 + 1);
    expect(mod.getRecentlyCompleted().has("coding:t1")).toBe(false);
  });
});

describe("ThreadEventBuffer — subscribe", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("drains buffered events first, then delivers live ones", async () => {
    const mod = await import("./ThreadEventBuffer");
    // Buffer 3 events for thread t1 BEFORE subscribing
    for (let i = 0; i < 3; i++) {
      mod.__testing.applyEvent({
        kind: "item_delta",
        thread_id: "coding:t1",
        turn_id: "turn-1",
        item_id: "i1",
        part_idx: 0,
        delta: { type: "text", append: `chunk-${i}` },
      });
    }
    const received: string[] = [];
    const unsubscribe = mod.subscribeToThread("coding:t1", (e) => {
      const d = (e as { delta?: { append?: string } }).delta;
      if (d?.append) received.push(d.append);
    });
    expect(received).toEqual(["chunk-0", "chunk-1", "chunk-2"]);

    // Now apply a live event
    mod.__testing.applyEvent({
      kind: "item_delta",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      item_id: "i1",
      part_idx: 0,
      delta: { type: "text", append: "chunk-live" },
    });
    expect(received).toEqual(["chunk-0", "chunk-1", "chunk-2", "chunk-live"]);

    unsubscribe();
    mod.__testing.applyEvent({
      kind: "item_delta",
      thread_id: "coding:t1",
      turn_id: "turn-1",
      item_id: "i1",
      part_idx: 0,
      delta: { type: "text", append: "after-unsub" },
    });
    expect(received).toEqual(["chunk-0", "chunk-1", "chunk-2", "chunk-live"]);
  });

  it("filters by threadId — events for other threads do not reach subscriber", async () => {
    const mod = await import("./ThreadEventBuffer");
    const received: string[] = [];
    mod.subscribeToThread("coding:a", (e) => {
      received.push((e as { thread_id?: string }).thread_id ?? "");
    });
    mod.__testing.applyEvent({
      kind: "turn_started",
      thread_id: "coding:b",
      turn_id: "x",
      model: "m",
      started_at: 0,
    });
    expect(received).toEqual([]);
  });
});

describe("ThreadEventBuffer — ring buffer", () => {
  beforeEach(async () => {
    const mod = await import("./ThreadEventBuffer");
    mod.__testing.reset();
  });

  it("caps events at 500 per thread (oldest dropped)", async () => {
    const mod = await import("./ThreadEventBuffer");
    for (let i = 0; i < 600; i++) {
      mod.__testing.applyEvent({
        kind: "item_delta",
        thread_id: "coding:t1",
        turn_id: "turn",
        item_id: "i",
        part_idx: 0,
        delta: { type: "text", append: `c${i}` },
      });
    }
    const received: string[] = [];
    mod.subscribeToThread("coding:t1", (e) => {
      const d = (e as { delta?: { append?: string } }).delta;
      if (d?.append) received.push(d.append);
    });
    expect(received).toHaveLength(500);
    // Oldest 100 dropped — first remaining is c100
    expect(received[0]).toBe("c100");
    expect(received[499]).toBe("c599");
  });
});
