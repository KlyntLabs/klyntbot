import { describe, it, expect } from "vitest";
import { applySubagentEvent } from "./useSubagents";
import type { SubagentEvent, SubagentActiveSummary } from "@/bindings";

const baseRow: SubagentActiveSummary = {
  agentId: "a1", label: "search", profile: "read_only",
  iteration: 0, status: "running", startedAt: 0,
  lastTool: null, durationMs: 0,
};

describe("applySubagentEvent", () => {
  it("adds row on spawned", () => {
    const e: SubagentEvent = {
      kind: "spawned", agent_id: "a1", label: "search",
      profile: "read_only", parent_session_id: "s1", spawned_at: 0,
    };
    const out = applySubagentEvent([], e);
    expect(out).toHaveLength(1);
    expect(out[0].agentId).toBe("a1");
  });

  it("updates iteration on progress", () => {
    const e: SubagentEvent = {
      kind: "progress", agent_id: "a1", iteration: 3, last_tool: "grep",
    };
    const out = applySubagentEvent([baseRow], e);
    expect(out[0].iteration).toBe(3);
    expect(out[0].lastTool).toBe("grep");
  });

  it("removes row on completed", () => {
    const e: SubagentEvent = {
      kind: "completed", agent_id: "a1", success: true,
      summary: "ok", tokens_used: 100, duration_ms: 500,
    };
    const out = applySubagentEvent([baseRow], e);
    expect(out).toHaveLength(0);
  });

  it("removes row on cancelled", () => {
    const e: SubagentEvent = {
      kind: "cancelled", agent_id: "a1",
      reason: "user_requested", cancelled_at: 0,
    };
    const out = applySubagentEvent([baseRow], e);
    expect(out).toHaveLength(0);
  });
});
