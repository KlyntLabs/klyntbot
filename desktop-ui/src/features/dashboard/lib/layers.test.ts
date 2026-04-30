import { describe, expect, it } from "vitest";
import { LAYERS } from "./layers";

describe("LAYERS config", () => {
  it("has 6 layers in fixed order", () => {
    expect(LAYERS.map((l) => l.key)).toEqual([
      "activity",
      "calendar",
      "timeEntries",
      "tasks",
      "transactions",
      "notes",
    ]);
  });

  it("activity layer maps to productivity + focus sources", () => {
    const activity = LAYERS.find((l) => l.key === "activity");
    expect(activity?.sources).toEqual(["productivity", "focus"]);
  });

  it("timeEntries is off by default", () => {
    expect(LAYERS.find((l) => l.key === "timeEntries")?.defaultOn).toBe(false);
  });
});
