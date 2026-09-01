import { describe, expect, it } from "vitest";
import { buildContainers, focusColor } from "./buildContainers";

describe("buildContainers", () => {
  it("returns empty array for no entries", () => {
    expect(buildContainers([])).toEqual([]);
  });

  it("returns empty array when entries have no durations and no point events", () => {
    expect(
      buildContainers([
        {
          id: "a",
          source: "task",
          entryType: "taskTimeEntry",
          title: "x",
          description: null,
          startedAt: "2026-04-30T09:00:00Z",
          endedAt: null,
          durationSecs: null,
          entityId: null,
          entityRoute: null,
          color: "#000",
          metadata: null,
        },
      ]),
    ).toEqual([]);
  });
});

describe("focusColor", () => {
  it("returns high-quality color for score > 7", () => {
    expect(focusColor(8)).toContain("--timeline-focus-high");
  });
  it("returns low-quality color for score < 4", () => {
    expect(focusColor(2)).toContain("--timeline-focus-low");
  });
  it("returns neutral color for mid-range", () => {
    expect(focusColor(5)).toContain("--timeline-focus");
  });
});
