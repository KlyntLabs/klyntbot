import { describe, expect, it } from "vitest";
import type { TimelineEntry } from "@/bindings";
import { computeDayStats, computeOverlapLayout, isActiveAppEntry } from "./timeline-utils";

function entry(overrides: Partial<TimelineEntry>): TimelineEntry {
  return {
    id: "x",
    source: "productivity",
    entryType: "appUsage",
    title: "Code",
    description: null,
    startedAt: "2026-04-30T09:00:00Z",
    endedAt: null,
    durationSecs: 600,
    entityId: null,
    entityRoute: null,
    color: "#000",
    metadata: null,
    ...overrides,
  };
}

describe("computeOverlapLayout", () => {
  it("returns empty map for empty input", () => {
    expect(computeOverlapLayout([])).toEqual(new Map());
  });

  it("places non-overlapping items in column 0", () => {
    const items = [
      { id: "a", startedAt: "2026-04-30T09:00:00Z", durationSecs: 600 },
      { id: "b", startedAt: "2026-04-30T10:00:00Z", durationSecs: 600 },
    ];
    const layout = computeOverlapLayout(items);
    expect(layout.get("a")).toEqual({ colIndex: 0, totalCols: 1 });
    expect(layout.get("b")).toEqual({ colIndex: 0, totalCols: 1 });
  });

  it("places overlapping items in side-by-side columns", () => {
    const items = [
      { id: "a", startedAt: "2026-04-30T09:00:00Z", durationSecs: 1800 },
      { id: "b", startedAt: "2026-04-30T09:15:00Z", durationSecs: 1800 },
    ];
    const layout = computeOverlapLayout(items);
    expect(layout.get("a")?.totalCols).toBe(2);
    expect(layout.get("b")?.totalCols).toBe(2);
    expect(layout.get("a")?.colIndex).not.toBe(layout.get("b")?.colIndex);
  });
});

describe("isActiveAppEntry", () => {
  it("returns false for idle apps", () => {
    expect(isActiveAppEntry(entry({ title: "loginwindow" }))).toBe(false);
  });
  it("returns true for productive apps with duration", () => {
    expect(isActiveAppEntry(entry({ title: "VSCode" }))).toBe(true);
  });
});

describe("computeDayStats", () => {
  it("sums focus and active seconds", () => {
    const entries = [
      entry({ source: "focus", entryType: "focusSession", durationSecs: 1800 }),
      entry({ source: "productivity", title: "VSCode", durationSecs: 600 }),
      entry({ source: "productivity", title: "loginwindow", durationSecs: 9999 }),
    ];
    const stats = computeDayStats(entries);
    expect(stats.focusSecs).toBe(1800);
    expect(stats.activeSecs).toBe(600);
  });
});
