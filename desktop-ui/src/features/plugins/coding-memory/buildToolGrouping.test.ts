import { describe, it, expect } from "vitest";
import { buildToolGrouping } from "./buildToolGrouping";
import type { WireEventDto } from "./types";

const ev = (idx: number, kind: string, payload: Record<string, unknown>): WireEventDto => ({
  id: `id-${idx}`,
  source: "kimiCli",
  sessionId: "s",
  kind,
  occurredAt: new Date(1700000000000 + idx * 1000).toISOString(),
  payloadDecoded: payload,
  rawJson: JSON.stringify(payload),
});

describe("buildToolGrouping", () => {
  it("links toolResult back to toolCall id", () => {
    const events = [
      ev(0, "toolCall", { id: "tc-1", function: { name: "Read" } }),
      ev(1, "toolResult", { tool_call_id: "tc-1" }),
    ];
    const meta = buildToolGrouping(events);
    expect(meta.get(0)?.linkedToolName).toBe("Read");
    expect(meta.get(1)?.linkedToolName).toBe("Read");
  });

  it("indents toolCallPart while a parent is active", () => {
    const events = [
      ev(0, "toolCall", { id: "tc-1", function: { name: "Bash" } }),
      ev(1, "toolCallPart", {}),
      ev(2, "toolResult", { tool_call_id: "tc-1" }),
    ];
    const meta = buildToolGrouping(events);
    expect(meta.get(1)?.nestLevel).toBe(1);
    expect(meta.get(0)?.nestLevel).toBe(0);
  });
});
