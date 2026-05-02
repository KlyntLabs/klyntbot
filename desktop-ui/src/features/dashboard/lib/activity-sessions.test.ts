import { describe, expect, it } from "vitest";
import { type MergeableEvent, mergeActivitySessions } from "./activity-sessions";

function ev(partial: Partial<MergeableEvent>): MergeableEvent {
  return {
    startSecs: 0,
    endSecs: 60,
    catType: "productive",
    color: "#22C55E",
    label: "VSCode",
    isIdle: false,
    dur: 60,
    ...partial,
  };
}

describe("mergeActivitySessions", () => {
  it("returns empty for empty input", () => {
    expect(mergeActivitySessions([])).toEqual([]);
  });

  it("skips idle events", () => {
    const result = mergeActivitySessions([ev({ isIdle: true })]);
    expect(result).toEqual([]);
  });

  it("merges adjacent events within 120s gap", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 0, endSecs: 60, dur: 60 }),
      ev({ startSecs: 100, endSecs: 200, dur: 100 }), // gap = 40s, < 120
    ]);
    expect(result).toHaveLength(1);
    expect(result[0].startSecs).toBe(0);
    expect(result[0].endSecs).toBe(200);
    expect(result[0].duration).toBe(200);
  });

  it("does NOT merge events with > 120s gap", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 0, endSecs: 60, dur: 60 }),
      ev({ startSecs: 200, endSecs: 300, dur: 100 }), // gap = 140s
    ]);
    expect(result).toHaveLength(2);
  });

  it("picks dominant category by total duration", () => {
    const result = mergeActivitySessions([
      ev({ catType: "productive", color: "#22C55E", dur: 60, label: "VSCode" }),
      ev({
        startSecs: 80,
        endSecs: 400,
        catType: "neutral",
        color: "#94A3B8",
        dur: 320,
        label: "Slack",
      }),
    ]);
    expect(result).toHaveLength(1);
    expect(result[0].dominantCategory).toBe("neutral");
    expect(result[0].color).toBe("#94A3B8");
  });

  it("picks best label by largest event duration", () => {
    const result = mergeActivitySessions([
      ev({ dur: 60, label: "Short" }),
      ev({ startSecs: 80, endSecs: 400, dur: 320, label: "Long" }),
    ]);
    expect(result[0].label).toBe("Long");
  });

  it("builds appBreakdown sorted by duration, max 5", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 0, endSecs: 30, dur: 30, label: "A" }),
      ev({ startSecs: 30, endSecs: 100, dur: 70, label: "B" }),
      ev({ startSecs: 100, endSecs: 110, dur: 10, label: "C" }),
    ]);
    expect(result[0].appBreakdown.map((a) => a.app)).toEqual(["B", "A", "C"]);
  });

  it("sorts unsorted input before merging", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 200, endSecs: 300, dur: 100 }),
      ev({ startSecs: 0, endSecs: 60, dur: 60 }),
    ]);
    // events are sorted, so 0..60 and 200..300 are processed in order; gap > 120, two sessions
    expect(result).toHaveLength(2);
    expect(result[0].startSecs).toBe(0);
    expect(result[1].startSecs).toBe(200);
  });

  it("drops zero-duration sessions", () => {
    const result = mergeActivitySessions([ev({ startSecs: 100, endSecs: 100, dur: 0 })]);
    expect(result).toEqual([]);
  });
});
