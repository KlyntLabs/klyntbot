import { describe, expect, it } from "vitest";
import { eventChipColor, formatTimeDelta, isErrorEvent } from "./eventHelpers";
import type { WireEventDto } from "./types";

const ev = (over: Partial<WireEventDto>): WireEventDto => ({
  id: "x",
  source: "kimiCli",
  sessionId: "s",
  kind: "toolCall",
  occurredAt: "2026-04-28T00:00:00Z",
  payloadDecoded: null,
  rawJson: "{}",
  ...over,
});

describe("eventChipColor", () => {
  it("maps tool kinds to purple", () => {
    expect(eventChipColor("toolCall")).toBe("purple");
    expect(eventChipColor("toolResult")).toBe("purple");
  });
  it("maps turn kinds to blue", () => {
    expect(eventChipColor("turnBegin")).toBe("blue");
    expect(eventChipColor("turnEnd")).toBe("blue");
  });
  it("maps step to green", () => {
    expect(eventChipColor("stepBegin")).toBe("green");
  });
  it("falls back to neutral for unknown", () => {
    expect(eventChipColor("ufoSighting")).toBe("neutral");
  });
});

describe("isErrorEvent", () => {
  it("flags error kind", () => {
    expect(isErrorEvent(ev({ kind: "error" }))).toBe(true);
  });
  it("flags toolResult.is_error payload", () => {
    expect(
      isErrorEvent(
        ev({
          kind: "toolResult",
          payloadDecoded: { return_value: { is_error: true } } as any,
        }),
      ),
    ).toBe(true);
  });
  it("does not flag plain toolResult", () => {
    expect(
      isErrorEvent(ev({ kind: "toolResult", payloadDecoded: { return_value: { ok: 1 } } as any })),
    ).toBe(false);
  });
});

describe("formatTimeDelta", () => {
  it("renders sub-second as ms", () => {
    expect(
      formatTimeDelta(new Date("2026-04-28T00:00:00.500Z"), new Date("2026-04-28T00:00:00.300Z")),
    ).toBe("+200ms");
  });
  it("renders seconds", () => {
    expect(
      formatTimeDelta(new Date("2026-04-28T00:00:30Z"), new Date("2026-04-28T00:00:00Z")),
    ).toBe("+30.00s");
  });
  it("returns empty for under-1ms", () => {
    expect(
      formatTimeDelta(new Date("2026-04-28T00:00:00.001Z"), new Date("2026-04-28T00:00:00.001Z")),
    ).toBe("");
  });
});
