import { describe, expect, it } from "vitest";
import { computeBfsWaves, selectHub } from "./graphBfs";

describe("selectHub", () => {
  it("returns activeNoteId when provided and exists in nodes", () => {
    const nodes = [
      { id: "a", linkCount: 1, title: "A" },
      { id: "b", linkCount: 5, title: "B" },
    ];
    expect(selectHub(nodes, "a")).toBe("a");
  });

  it("returns most-connected node when no activeNoteId", () => {
    const nodes = [
      { id: "a", linkCount: 1, title: "A" },
      { id: "b", linkCount: 5, title: "B" },
      { id: "c", linkCount: 3, title: "C" },
    ];
    expect(selectHub(nodes, null)).toBe("b");
  });

  it("breaks ties alphabetically by title", () => {
    const nodes = [
      { id: "x", linkCount: 3, title: "Zebra" },
      { id: "y", linkCount: 3, title: "Alpha" },
    ];
    expect(selectHub(nodes, null)).toBe("y");
  });

  it("returns first node as fallback for empty linkCounts", () => {
    const nodes = [
      { id: "a", linkCount: 0, title: "B" },
      { id: "b", linkCount: 0, title: "A" },
    ];
    expect(selectHub(nodes, null)).toBe("b"); // alphabetical by title
  });
});

describe("computeBfsWaves", () => {
  it("returns hub as wave 0", () => {
    const waves = computeBfsWaves("a", new Map([["a", new Set(["b"])], ["b", new Set(["a"])]]), new Set(["a", "b"]));
    expect(waves[0]).toEqual(["a"]);
  });

  it("returns direct neighbors as wave 1", () => {
    const adj = new Map([
      ["a", new Set(["b", "c"])],
      ["b", new Set(["a"])],
      ["c", new Set(["a"])],
    ]);
    const waves = computeBfsWaves("a", adj, new Set(["a", "b", "c"]));
    expect(waves[0]).toEqual(["a"]);
    expect(waves[1]?.sort()).toEqual(["b", "c"]);
  });

  it("puts orphans in the last wave", () => {
    const adj = new Map([["a", new Set(["b"])], ["b", new Set(["a"])]]);
    const allNodeIds = new Set(["a", "b", "orphan1", "orphan2"]);
    const waves = computeBfsWaves("a", adj, allNodeIds);
    const lastWave = waves[waves.length - 1];
    expect(lastWave?.sort()).toEqual(["orphan1", "orphan2"]);
  });

  it("handles single-node graph", () => {
    const waves = computeBfsWaves("a", new Map(), new Set(["a"]));
    expect(waves).toEqual([["a"]]);
  });

  it("handles disconnected components", () => {
    const adj = new Map([
      ["a", new Set(["b"])],
      ["b", new Set(["a"])],
      ["c", new Set(["d"])],
      ["d", new Set(["c"])],
    ]);
    const allNodeIds = new Set(["a", "b", "c", "d"]);
    const waves = computeBfsWaves("a", adj, allNodeIds);
    expect(waves[0]).toEqual(["a"]);
    expect(waves[1]).toEqual(["b"]);
    // c and d are unreachable from a — they end up in the last wave
    const lastWave = waves[waves.length - 1];
    expect(lastWave?.sort()).toEqual(["c", "d"]);
  });
});
