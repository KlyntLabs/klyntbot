import { describe, expect, it } from "vitest";
import { computeFingerprint } from "./graphFingerprint";

describe("computeFingerprint", () => {
  it("returns same hash for same nodes and edges regardless of order", () => {
    const a = computeFingerprint(
      ["b", "a", "c"],
      [
        ["a", "b"],
        ["b", "c"],
      ],
    );
    const b = computeFingerprint(
      ["c", "a", "b"],
      [
        ["b", "c"],
        ["a", "b"],
      ],
    );
    expect(a).toBe(b);
  });

  it("returns different hash when a node is added", () => {
    const a = computeFingerprint(["a", "b"], [["a", "b"]]);
    const b = computeFingerprint(["a", "b", "c"], [["a", "b"]]);
    expect(a).not.toBe(b);
  });

  it("returns different hash when an edge is added", () => {
    const a = computeFingerprint(["a", "b", "c"], [["a", "b"]]);
    const b = computeFingerprint(
      ["a", "b", "c"],
      [
        ["a", "b"],
        ["b", "c"],
      ],
    );
    expect(a).not.toBe(b);
  });

  it("normalizes edge direction (a->b same as b->a)", () => {
    const a = computeFingerprint(["a", "b"], [["a", "b"]]);
    const b = computeFingerprint(["a", "b"], [["b", "a"]]);
    expect(a).toBe(b);
  });

  it("handles empty graph", () => {
    const result = computeFingerprint([], []);
    expect(typeof result).toBe("string");
    expect(result.length).toBeGreaterThan(0);
  });
});
