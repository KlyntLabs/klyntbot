import { describe, it, expect } from "vitest";
import { applyThreadEvent } from "./useThreadEvents";
import type { ThreadEvent, MessageDto } from "./useThreadEvents";

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

describe("applyThreadEvent", () => {
  it("turn_started → streaming state", () => {
    const e: ThreadEvent = {
      kind: "turn_started",
      thread_id: "t",
      turn_id: "turn-1",
      model: "gpt-5",
      started_at: 0,
    };
    const s = applyThreadEvent({ items: [], turnState: { kind: "idle" } }, e);
    expect(s.turnState).toEqual({ kind: "streaming", turnId: "turn-1", model: "gpt-5" });
  });

  it("item_started appends to items", () => {
    const item = baseItem("m1");
    const e: ThreadEvent = { kind: "item_started", thread_id: "t", turn_id: "turn-1", item };
    const s = applyThreadEvent({ items: [], turnState: { kind: "idle" } }, e);
    expect(s.items).toEqual([item]);
  });

  it("item_delta text appends to existing text part at same idx", () => {
    const item: MessageDto = {
      ...baseItem("m1"),
      parts: [{ kind: "text", text: "Hel" }],
    };
    const init = { items: [item], turnState: { kind: "idle" } as const };
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

  it("turn_completed → idle with finish reason", () => {
    const e: ThreadEvent = {
      kind: "turn_completed",
      thread_id: "t",
      turn_id: "turn-1",
      finish_reason: { kind: "completed" },
      completed_at: 1,
      duration_ms: 100,
    };
    const s = applyThreadEvent(
      { items: [], turnState: { kind: "streaming", turnId: "turn-1", model: "x" } },
      e,
    );
    expect(s.turnState).toEqual({ kind: "idle", lastFinishReason: { kind: "completed" } });
  });

  it("ignores unknown event kinds without throwing", () => {
    const e = { kind: "future_event", thread_id: "t" } as unknown as ThreadEvent;
    const init = { items: [], turnState: { kind: "idle" as const } };
    expect(applyThreadEvent(init, e)).toBe(init);
  });
});
