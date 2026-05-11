import { describe, expect, it } from "vitest";
import type { MessageDto, ThreadEvent } from "./useThreadEvents";
import { applyThreadEvent } from "./useThreadEvents";

const baseItem = (id: string): MessageDto => ({
  id,
  session_id: "t",
  role: "assistant",
  parts: [],
  model: null,
  turn_id: "turn-1",
  created_at: 0,
  finish_reason: null,
});

const idleState = () => ({
  items: [] as MessageDto[],
  turnState: { kind: "idle" as const },
  processingStartedAt: null,
  lastDurationMs: null,
});

describe("applyThreadEvent", () => {
  it("turn_started → streaming state with start timestamp", () => {
    const e: ThreadEvent = {
      kind: "turn_started",
      thread_id: "t",
      turn_id: "turn-1",
      model: "gpt-5",
      started_at: 1700,
    };
    const s = applyThreadEvent(idleState(), e);
    expect(s.turnState).toEqual({
      kind: "streaming",
      turnId: "turn-1",
      model: "gpt-5",
      startedAt: 1700,
    });
    expect(s.processingStartedAt).toBe(1700);
    expect(s.lastDurationMs).toBeNull();
  });

  it("item_started appends to items", () => {
    const item = baseItem("m1");
    const e: ThreadEvent = { kind: "item_started", thread_id: "t", turn_id: "turn-1", item };
    const s = applyThreadEvent(idleState(), e);
    expect(s.items).toEqual([item]);
  });

  it("item_delta text appends to existing text part at same idx", () => {
    const item: MessageDto = {
      ...baseItem("m1"),
      parts: [{ kind: "text", text: "Hel" }],
    };
    const init = { ...idleState(), items: [item] };
    const e: ThreadEvent = {
      kind: "item_delta",
      thread_id: "t",
      turn_id: "turn-1",
      item_id: "m1",
      part_idx: 0,
      delta: { type: "text", append: "lo" },
    };
    const s = applyThreadEvent(init, e);
    expect(s.items[0].parts[0]).toEqual({ kind: "text", text: "Hello" });
  });

  it("tool_call_started preserves processingStartedAt so the timer keeps ticking", () => {
    const init = {
      ...idleState(),
      turnState: {
        kind: "streaming" as const,
        turnId: "turn-1",
        model: "x",
        startedAt: 1700,
      },
      processingStartedAt: 1700,
    };
    const e: ThreadEvent = {
      kind: "tool_call_started",
      thread_id: "t",
      turn_id: "turn-1",
      item_id: "m1",
      call_id: "call-1",
      tool: "memory",
    };
    const s = applyThreadEvent(init, e);
    expect(s.processingStartedAt).toBe(1700);
    expect(s.turnState).toMatchObject({ kind: "tool_executing", startedAt: 1700 });
  });

  it("turn_completed → idle, clears processingStartedAt, captures lastDurationMs", () => {
    const e: ThreadEvent = {
      kind: "turn_completed",
      thread_id: "t",
      turn_id: "turn-1",
      finish_reason: { kind: "completed" },
      completed_at: 1,
      duration_ms: 12345,
    };
    const s = applyThreadEvent(
      {
        ...idleState(),
        turnState: { kind: "streaming", turnId: "turn-1", model: "x", startedAt: 0 },
        processingStartedAt: 0,
      },
      e,
    );
    expect(s.turnState).toEqual({ kind: "idle", lastFinishReason: { kind: "completed" } });
    expect(s.processingStartedAt).toBeNull();
    expect(s.lastDurationMs).toBe(12345);
  });

  it("ignores unknown event kinds without throwing", () => {
    const e = { kind: "future_event", thread_id: "t" } as unknown as ThreadEvent;
    const init = idleState();
    expect(applyThreadEvent(init, e)).toBe(init);
  });
});
