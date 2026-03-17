import type { ElementDefinition } from "cytoscape";

export interface ElementDiffResult {
  addedNodes: ElementDefinition[];
  removedNodeIds: string[];
  addedEdges: ElementDefinition[];
  removedEdgeIds: string[];
  hasChanges: boolean;
}

/**
 * Compute a surgical diff between two Cytoscape element arrays.
 * Returns the minimal set of add/remove operations needed.
 */
export function diffElements(
  prev: ElementDefinition[],
  next: ElementDefinition[],
): ElementDiffResult {
  const prevNodes = new Map<string, ElementDefinition>();
  const prevEdges = new Map<string, ElementDefinition>();
  const nextNodes = new Map<string, ElementDefinition>();
  const nextEdges = new Map<string, ElementDefinition>();

  for (const el of prev) {
    const id = el.data?.id;
    if (!id) continue;
    if (el.group === "edges") {
      prevEdges.set(id, el);
    } else {
      prevNodes.set(id, el);
    }
  }

  for (const el of next) {
    const id = el.data?.id;
    if (!id) continue;
    if (el.group === "edges") {
      nextEdges.set(id, el);
    } else {
      nextNodes.set(id, el);
    }
  }

  const addedNodes: ElementDefinition[] = [];
  const removedNodeIds: string[] = [];
  const addedEdges: ElementDefinition[] = [];
  const removedEdgeIds: string[] = [];

  for (const [id, el] of nextNodes) {
    if (!prevNodes.has(id)) addedNodes.push(el);
  }
  for (const id of prevNodes.keys()) {
    if (!nextNodes.has(id)) removedNodeIds.push(id);
  }
  for (const [id, el] of nextEdges) {
    if (!prevEdges.has(id)) addedEdges.push(el);
  }
  for (const id of prevEdges.keys()) {
    if (!nextEdges.has(id)) removedEdgeIds.push(id);
  }

  const hasChanges =
    addedNodes.length > 0 ||
    removedNodeIds.length > 0 ||
    addedEdges.length > 0 ||
    removedEdgeIds.length > 0;

  return { addedNodes, removedNodeIds, addedEdges, removedEdgeIds, hasChanges };
}
