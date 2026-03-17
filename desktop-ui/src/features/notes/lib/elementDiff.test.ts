import { describe, expect, it } from "vitest";
import type { ElementDefinition } from "cytoscape";
import { diffElements } from "./elementDiff";

const node = (id: string): ElementDefinition => ({
  group: "nodes",
  data: { id, label: id },
});

const edge = (source: string, target: string): ElementDefinition => ({
  group: "edges",
  data: { id: `e:${source}:${target}`, source, target },
});

describe("diffElements", () => {
  it("detects added nodes", () => {
    const prev = [node("a"), node("b")];
    const next = [node("a"), node("b"), node("c")];
    const diff = diffElements(prev, next);
    expect(diff.addedNodes.map((e) => e.data.id)).toEqual(["c"]);
    expect(diff.removedNodeIds).toEqual([]);
  });

  it("detects removed nodes", () => {
    const prev = [node("a"), node("b"), node("c")];
    const next = [node("a"), node("b")];
    const diff = diffElements(prev, next);
    expect(diff.removedNodeIds).toEqual(["c"]);
    expect(diff.addedNodes).toEqual([]);
  });

  it("detects added edges", () => {
    const prev = [node("a"), node("b"), edge("a", "b")];
    const next = [node("a"), node("b"), node("c"), edge("a", "b"), edge("b", "c")];
    const diff = diffElements(prev, next);
    expect(diff.addedEdges.map((e) => e.data.id)).toEqual(["e:b:c"]);
  });

  it("detects removed edges", () => {
    const prev = [node("a"), node("b"), edge("a", "b")];
    const next = [node("a"), node("b")];
    const diff = diffElements(prev, next);
    expect(diff.removedEdgeIds).toEqual(["e:a:b"]);
  });

  it("returns empty diff for identical elements", () => {
    const els = [node("a"), node("b"), edge("a", "b")];
    const diff = diffElements(els, els);
    expect(diff.addedNodes).toEqual([]);
    expect(diff.removedNodeIds).toEqual([]);
    expect(diff.addedEdges).toEqual([]);
    expect(diff.removedEdgeIds).toEqual([]);
    expect(diff.hasChanges).toBe(false);
  });

  it("hasChanges is true when there are changes", () => {
    const diff = diffElements([node("a")], [node("a"), node("b")]);
    expect(diff.hasChanges).toBe(true);
  });
});
