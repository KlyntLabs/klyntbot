# Graph Progressive Loading & Interactive Physics — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the all-at-once graph rendering with progressive BFS loading, position caching, interactive Cola drag physics, and localized element diffing — so the graph feels alive, scales to thousands of nodes, and doesn't rearrange on every update.

**Architecture:** Hybrid fCoSE + Cola. fCoSE computes initial static layouts (compound-aware, fast). Cola provides interactive drag physics via `node.lock()`/`unlock()` scoping. A shared IndexedDB position cache bridges both — fCoSE writes to it, Cola reads/writes to it, and the graph always starts from cached positions when available. Element changes use surgical `cy.add()`/`cy.remove()` instead of the current nuclear `cy.json({ elements })`.

**Tech Stack:** Cytoscape.js, cytoscape-fcose (existing), cytoscape-cola (new), IndexedDB (via idb-keyval or raw API), React hooks, Vitest

**Spec:** `docs/superpowers/specs/2026-03-17-graph-progressive-physics-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/lib/graphFingerprint.ts` | Pure function: compute deterministic hash from node IDs + edge pairs |
| `desktop-ui/src/features/notes/lib/graphFingerprint.test.ts` | Tests for fingerprint computation |
| `desktop-ui/src/features/notes/lib/graphBfs.ts` | Pure function: BFS wave generation from a hub node |
| `desktop-ui/src/features/notes/lib/graphBfs.test.ts` | Tests for BFS wave computation |
| `desktop-ui/src/features/notes/lib/elementDiff.ts` | Pure function: diff two Cytoscape element arrays into add/remove sets |
| `desktop-ui/src/features/notes/lib/elementDiff.test.ts` | Tests for element diffing |
| `desktop-ui/src/features/notes/lib/graphUtils.ts` | Shared utilities: `snapshotPositions()`, Cytoscape plugin registration |
| `desktop-ui/src/features/notes/lib/cytoscape-cola.d.ts` | Type declaration shim for `cytoscape-cola` (no `@types` package exists) |
| `desktop-ui/src/features/notes/hooks/useGraphPositionCache.ts` | Hook: IndexedDB read/write for position maps, keyed by viewMode + fingerprint |
| `desktop-ui/src/features/notes/hooks/useProgressiveReveal.ts` | Hook: orchestrate wave-based reveal animation on Cytoscape instance |
| `desktop-ui/src/features/notes/hooks/useColaPhysics.ts` | Hook: Cola drag activation, Live Physics toggle, lock/unlock scoping |

### Modified files

| File | Changes |
|------|---------|
| `desktop-ui/src/features/notes/hooks/useCytoscapeGraph.ts` | Major refactor: integrate position cache, element diffing, remove `cy.json()`. Wire progressive reveal and Cola hooks. |
| `desktop-ui/src/features/notes/hooks/useCytoscapeElements.ts` | Export fingerprint alongside elements |
| `desktop-ui/src/features/notes/hooks/useGraphSettings.ts` | Add `livePhysics` and `instantLoad` settings |
| `desktop-ui/src/features/notes/hooks/useCytoscapeTheme.ts` | Add Cola visual feedback styles (drag halo, neighbor glow, hub pulse) |
| `desktop-ui/src/features/notes/components/GraphView.tsx` | Wire new hooks, add Live Physics toolbar button |
| `desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx` | Add "Instant Load" toggle |
| `desktop-ui/package.json` | Add `cytoscape-cola` dependency |

---

## Task 1: Install cytoscape-cola & Add Graph Fingerprint

**Files:**
- Modify: `desktop-ui/package.json`
- Create: `desktop-ui/src/features/notes/lib/graphFingerprint.ts`
- Create: `desktop-ui/src/features/notes/lib/graphFingerprint.test.ts`
- Modify: `desktop-ui/src/features/notes/hooks/useCytoscapeElements.ts:49-167`

- [ ] **Step 1: Install cytoscape-cola**

```bash
cd desktop-ui && bun add cytoscape-cola
```

- [ ] **Step 2: Create cytoscape-cola type declaration + shared utils**

Create `desktop-ui/src/features/notes/lib/cytoscape-cola.d.ts`:

```ts
declare module "cytoscape-cola" {
  import type { Ext } from "cytoscape";
  const ext: Ext;
  export default ext;
}
```

Create `desktop-ui/src/features/notes/lib/graphUtils.ts`:

```ts
import type { Core } from "cytoscape";
import cytoscape from "cytoscape";
import fcose from "cytoscape-fcose";
import cola from "cytoscape-cola";

// Register Cytoscape plugins once. Multiple calls to cytoscape.use()
// with the same plugin are safe (Cytoscape ignores duplicates for
// built-in layouts, but extensions may warn). Guard with a flag.
let pluginsRegistered = false;
export function registerCytoscapePlugins() {
  if (pluginsRegistered) return;
  cytoscape.use(fcose);
  cytoscape.use(cola);
  pluginsRegistered = true;
}

export interface PositionEntry {
  x: number;
  y: number;
}

export type PositionMap = Record<string, PositionEntry>;

/** Snapshot all leaf node positions from a Cytoscape instance. */
export function snapshotPositions(cy: Core): PositionMap {
  const positions: PositionMap = {};
  cy.nodes(":childless").forEach((node) => {
    const pos = node.position();
    positions[node.id()] = { x: pos.x, y: pos.y };
  });
  return positions;
}
```

- [ ] **Step 3: Write failing tests for graphFingerprint**

Create `desktop-ui/src/features/notes/lib/graphFingerprint.test.ts` (uses `|` separator internally to avoid conflicts with node IDs that may contain `:`):

```ts
import { describe, expect, it } from "vitest";
import { computeFingerprint } from "./graphFingerprint";

describe("computeFingerprint", () => {
  it("returns same hash for same nodes and edges regardless of order", () => {
    const a = computeFingerprint(["b", "a", "c"], [["a", "b"], ["b", "c"]]);
    const b = computeFingerprint(["c", "a", "b"], [["b", "c"], ["a", "b"]]);
    expect(a).toBe(b);
  });

  it("returns different hash when a node is added", () => {
    const a = computeFingerprint(["a", "b"], [["a", "b"]]);
    const b = computeFingerprint(["a", "b", "c"], [["a", "b"]]);
    expect(a).not.toBe(b);
  });

  it("returns different hash when an edge is added", () => {
    const a = computeFingerprint(["a", "b", "c"], [["a", "b"]]);
    const b = computeFingerprint(["a", "b", "c"], [["a", "b"], ["b", "c"]]);
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
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cd desktop-ui && bun run test -- graphFingerprint
```
Expected: FAIL — module not found.

- [ ] **Step 4: Implement graphFingerprint**

Create `desktop-ui/src/features/notes/lib/graphFingerprint.ts`:

```ts
/**
 * Compute a deterministic fingerprint from graph structure.
 * Used as cache key for position persistence — changes when nodes/edges change.
 */
export function computeFingerprint(
  nodeIds: string[],
  edgePairs: [string, string][],
): string {
  const sortedNodes = [...nodeIds].sort().join(",");
  const sortedEdges = edgePairs
    .map(([a, b]) => [a, b].sort().join("\x00"))
    .sort()
    .join(",");
  const raw = `${sortedNodes}|${sortedEdges}`;
  // Simple djb2 hash — fast and sufficient for cache key comparison
  let hash = 5381;
  for (let i = 0; i < raw.length; i++) {
    hash = ((hash << 5) + hash + raw.charCodeAt(i)) >>> 0;
  }
  return hash.toString(36);
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test -- graphFingerprint
```
Expected: All 5 tests PASS.

- [ ] **Step 6: Export fingerprint from useCytoscapeElements**

Modify `desktop-ui/src/features/notes/hooks/useCytoscapeElements.ts` — import `computeFingerprint` and include it in the return value. The `useMemo` should compute and return `fingerprint` alongside `elements` and `clusters`.

At the top, add:
```ts
import { computeFingerprint } from "../lib/graphFingerprint";
```

Change the return type to include `fingerprint: string` and add this before the final `return`:
```ts
    // Compute structural fingerprint for position cache keying
    const nodeIdList = nodes.map((n) => n.id);
    // Extract edge pairs from the deduplicated edge elements (not from seenEdges keys,
    // which use ":" separator that could conflict with node IDs containing colons)
    const edgePairList: [string, string][] = elements
      .filter((el) => el.group === "edges")
      .map((el) => [el.data?.source as string, el.data?.target as string]);
    const fingerprint = computeFingerprint(nodeIdList, edgePairList);

    return { elements, clusters, fingerprint };
```

Update the function's return type annotation:
```ts
): { elements: ElementDefinition[]; clusters: ClusterInfo[]; fingerprint: string }
```

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/notes/lib/graphFingerprint.ts desktop-ui/src/features/notes/lib/graphFingerprint.test.ts desktop-ui/src/features/notes/lib/graphUtils.ts desktop-ui/src/features/notes/lib/cytoscape-cola.d.ts desktop-ui/src/features/notes/hooks/useCytoscapeElements.ts desktop-ui/package.json desktop-ui/bun.lock
git commit -m "feat(notes): add graph fingerprint, shared utils, install cytoscape-cola"
```

---

## Task 2: BFS Wave Generation

**Files:**
- Create: `desktop-ui/src/features/notes/lib/graphBfs.ts`
- Create: `desktop-ui/src/features/notes/lib/graphBfs.test.ts`

- [ ] **Step 1: Write failing tests for BFS wave generation**

Create `desktop-ui/src/features/notes/lib/graphBfs.test.ts`:

```ts
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
    // a in wave 0, b in wave 1, c+d end up in orphan wave (disconnected from hub)
    expect(waves[0]).toEqual(["a"]);
    expect(waves[1]).toEqual(["b"]);
    // c and d are unreachable from a, so they're in the last wave
    const lastWave = waves[waves.length - 1];
    expect(lastWave?.sort()).toEqual(["c", "d"]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd desktop-ui && bun run test -- graphBfs
```
Expected: FAIL — module not found.

- [ ] **Step 3: Implement graphBfs**

Create `desktop-ui/src/features/notes/lib/graphBfs.ts`:

```ts
interface HubCandidate {
  id: string;
  linkCount: number;
  title: string;
}

/**
 * Select the hub node (center of the graph).
 * Priority: activeNoteId > most-connected > alphabetical fallback.
 */
export function selectHub(
  nodes: HubCandidate[],
  activeNoteId: string | null,
): string {
  if (activeNoteId && nodes.some((n) => n.id === activeNoteId)) {
    return activeNoteId;
  }
  const sorted = [...nodes].sort((a, b) => {
    if (b.linkCount !== a.linkCount) return b.linkCount - a.linkCount;
    return a.title.localeCompare(b.title);
  });
  return sorted[0]?.id ?? "";
}

/**
 * Compute BFS waves from a hub node outward.
 * Returns array of waves, where each wave is a list of node IDs.
 * Nodes unreachable from the hub (orphans + disconnected components)
 * are placed in the final wave.
 */
export function computeBfsWaves(
  hubId: string,
  adjacency: Map<string, Set<string>>,
  allNodeIds: Set<string>,
): string[][] {
  const waves: string[][] = [];
  const visited = new Set<string>();

  // BFS from hub
  let currentWave = [hubId];
  visited.add(hubId);

  while (currentWave.length > 0) {
    waves.push(currentWave);
    const nextWave: string[] = [];
    for (const nodeId of currentWave) {
      const neighbors = adjacency.get(nodeId);
      if (!neighbors) continue;
      for (const neighbor of neighbors) {
        if (!visited.has(neighbor) && allNodeIds.has(neighbor)) {
          visited.add(neighbor);
          nextWave.push(neighbor);
        }
      }
    }
    currentWave = nextWave;
  }

  // Collect orphans + disconnected nodes
  const remaining: string[] = [];
  for (const id of allNodeIds) {
    if (!visited.has(id)) {
      remaining.push(id);
    }
  }
  if (remaining.length > 0) {
    waves.push(remaining);
  }

  return waves;
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test -- graphBfs
```
Expected: All 9 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/lib/graphBfs.ts desktop-ui/src/features/notes/lib/graphBfs.test.ts
git commit -m "feat(notes): add BFS wave generation for progressive graph reveal"
```

---

## Task 3: Element Diffing

**Files:**
- Create: `desktop-ui/src/features/notes/lib/elementDiff.ts`
- Create: `desktop-ui/src/features/notes/lib/elementDiff.test.ts`

- [ ] **Step 1: Write failing tests for element diffing**

Create `desktop-ui/src/features/notes/lib/elementDiff.test.ts`:

```ts
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
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd desktop-ui && bun run test -- elementDiff
```
Expected: FAIL — module not found.

- [ ] **Step 3: Implement elementDiff**

Create `desktop-ui/src/features/notes/lib/elementDiff.ts`:

```ts
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

  // Nodes added
  for (const [id, el] of nextNodes) {
    if (!prevNodes.has(id)) addedNodes.push(el);
  }
  // Nodes removed
  for (const id of prevNodes.keys()) {
    if (!nextNodes.has(id)) removedNodeIds.push(id);
  }
  // Edges added
  for (const [id, el] of nextEdges) {
    if (!prevEdges.has(id)) addedEdges.push(el);
  }
  // Edges removed
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
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test -- elementDiff
```
Expected: All 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/lib/elementDiff.ts desktop-ui/src/features/notes/lib/elementDiff.test.ts
git commit -m "feat(notes): add surgical element diffing for graph updates"
```

---

## Task 4: Position Cache (IndexedDB)

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useGraphPositionCache.ts`

- [ ] **Step 1: Implement useGraphPositionCache**

Create `desktop-ui/src/features/notes/hooks/useGraphPositionCache.ts`:

```ts
import { useCallback, useEffect, useRef } from "react";

import type { PositionMap } from "../lib/graphUtils";

interface CacheEntry {
  key: string;
  positions: PositionMap;
  timestamp: number;
}

const DB_NAME = "klynt-graph-positions";
const STORE_NAME = "positions";
const DB_VERSION = 1;
const MAX_ENTRIES = 15; // ~3 entries per 5 view modes

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: "key" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function readFromIDB(key: string): Promise<PositionMap | null> {
  try {
    const db = await openDB();
    return new Promise((resolve) => {
      const tx = db.transaction(STORE_NAME, "readonly");
      const store = tx.objectStore(STORE_NAME);
      const req = store.get(key);
      req.onsuccess = () => {
        const entry = req.result as CacheEntry | undefined;
        resolve(entry?.positions ?? null);
      };
      req.onerror = () => resolve(null);
    });
  } catch {
    return null;
  }
}

async function writeToIDB(key: string, positions: PositionMap): Promise<void> {
  try {
    const db = await openDB();
    // Use a single readwrite transaction for both put + eviction
    // so the transaction stays alive through all chained requests.
    const tx = db.transaction(STORE_NAME, "readwrite");
    const store = tx.objectStore(STORE_NAME);
    // First count, then put, then evict — all on the same transaction
    const countReq = store.count();
    await new Promise<void>((resolve, reject) => {
      countReq.onsuccess = () => {
        store.put({ key, positions, timestamp: Date.now() } satisfies CacheEntry);
        const total = countReq.result + 1; // +1 for the put we just did
        if (total > MAX_ENTRIES) {
          const toDelete = total - MAX_ENTRIES;
          let deleted = 0;
          const cursor = store.openCursor();
          cursor.onsuccess = () => {
            const c = cursor.result;
            if (c && deleted < toDelete) {
              c.delete();
              deleted++;
              c.continue();
            } else {
              resolve();
            }
          };
          cursor.onerror = () => reject(cursor.error);
        } else {
          resolve();
        }
      };
      countReq.onerror = () => reject(countReq.error);
    });
  } catch {
    // Fallback: try localStorage for small graphs
    try {
      const lsKey = `graph-pos:${key}`;
      localStorage.setItem(lsKey, JSON.stringify(positions));
    } catch {
      // Silently fail — graph will just re-layout
    }
  }
}

function buildCacheKey(viewMode: string, fingerprint: string): string {
  return `${viewMode}-${fingerprint}`;
}

/**
 * Position cache hook. Provides load/save operations for graph node positions.
 * Primary storage: IndexedDB. Fallback: localStorage.
 */
export function useGraphPositionCache(viewMode: string, fingerprint: string) {
  const cacheKeyRef = useRef(buildCacheKey(viewMode, fingerprint));
  cacheKeyRef.current = buildCacheKey(viewMode, fingerprint);

  const loadPositions = useCallback(async (): Promise<PositionMap | null> => {
    const key = cacheKeyRef.current;
    // Try IndexedDB first
    const idbResult = await readFromIDB(key);
    if (idbResult) return idbResult;
    // Fallback: localStorage
    try {
      const stored = localStorage.getItem(`graph-pos:${key}`);
      if (stored) return JSON.parse(stored) as PositionMap;
    } catch {
      // ignore
    }
    return null;
  }, []);

  const savePositions = useCallback(async (positions: PositionMap): Promise<void> => {
    const key = cacheKeyRef.current;
    await writeToIDB(key, positions);
  }, []);

  const clearPositions = useCallback(async (): Promise<void> => {
    const key = cacheKeyRef.current;
    try {
      const db = await openDB();
      const tx = db.transaction(STORE_NAME, "readwrite");
      tx.objectStore(STORE_NAME).delete(key);
    } catch {
      // ignore
    }
    try {
      localStorage.removeItem(`graph-pos:${key}`);
    } catch {
      // ignore
    }
  }, []);

  // Invalidate cache when fingerprint changes (new key = automatic miss)
  useEffect(() => {
    cacheKeyRef.current = buildCacheKey(viewMode, fingerprint);
  }, [viewMode, fingerprint]);

  return { loadPositions, savePositions, clearPositions };
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```
Expected: No errors in `useGraphPositionCache.ts`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphPositionCache.ts
git commit -m "feat(notes): add IndexedDB position cache for graph layout persistence"
```

---

## Task 5: Update Graph Settings (livePhysics + instantLoad)

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useGraphSettings.ts:1-67`
- Modify: `desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx:91-184`

- [ ] **Step 1: Add new settings to GraphSettings interface**

In `desktop-ui/src/features/notes/hooks/useGraphSettings.ts`, add two new fields to the `GraphSettings` interface:

```ts
export interface GraphSettings {
  linkDistance: number;
  repulsion: number;
  centerForce: number;
  nodeScale: number;
  labelThreshold: number;
  showArrows: boolean;
  showOrphans: boolean;
  /** Enable continuous physics simulation (CPU-intensive) */
  livePhysics: boolean;
  /** Skip progressive reveal animation on graph load */
  instantLoad: boolean;
}
```

Add defaults:
```ts
const DEFAULT_SETTINGS: GraphSettings = {
  linkDistance: 120,
  repulsion: 8000,
  centerForce: 0.2,
  nodeScale: 1,
  labelThreshold: 0.5,
  showArrows: true,
  showOrphans: true,
  livePhysics: false,
  instantLoad: false,
};
```

- [ ] **Step 2: Add "Instant Load" toggle to GraphSettingsPopover**

In `desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx`, add after the "Show Orphan Nodes" toggle:

```tsx
        <Toggle
          label="Instant Load"
          checked={settings.instantLoad}
          onChange={(v) => onChange({ instantLoad: v })}
        />
```

- [ ] **Step 3: Verify it compiles and renders**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphSettings.ts desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx
git commit -m "feat(notes): add livePhysics and instantLoad graph settings"
```

---

## Task 6: Progressive Reveal Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useProgressiveReveal.ts`

- [ ] **Step 1: Implement useProgressiveReveal**

Create `desktop-ui/src/features/notes/hooks/useProgressiveReveal.ts`:

```ts
import type { Core, ElementDefinition } from "cytoscape";
import { useCallback, useRef } from "react";
import type { PositionMap } from "../lib/graphUtils";

interface RevealOptions {
  /** Milliseconds between waves (cache hit = fast, cache miss = slow) */
  waveDelay: number;
  /** Maximum number of animated waves before batching the rest */
  maxWaves: number;
  /** Whether to skip animation entirely */
  instant: boolean;
}

/**
 * Orchestrate wave-based progressive reveal on a Cytoscape instance.
 * Nodes are added in BFS waves with staggered opacity/scale animation.
 */
export function useProgressiveReveal() {
  const animationRef = useRef<number | null>(null);
  const isRevealingRef = useRef(false);

  const cancelReveal = useCallback(() => {
    if (animationRef.current !== null) {
      clearTimeout(animationRef.current);
      animationRef.current = null;
    }
    isRevealingRef.current = false;
  }, []);

  /**
   * Reveal elements wave by wave on a cache-hit (positions already known).
   * Purely visual — no layout computation, just staggered opacity + scale.
   */
  const revealWithPositions = useCallback(
    (
      cy: Core,
      waves: string[][],
      allElements: ElementDefinition[],
      positions: PositionMap,
      options: RevealOptions,
    ) => {
      cancelReveal();

      if (options.instant) {
        // Add all elements at once with cached positions
        const positioned = allElements.map((el) => {
          const pos = el.data?.id ? positions[el.data.id] : undefined;
          if (pos && el.group !== "edges") {
            return { ...el, position: pos };
          }
          return el;
        });
        cy.add(positioned);
        cy.nodes(":parent").ungrabify().unselectify();
        cy.fit(undefined, 40);
        return;
      }

      isRevealingRef.current = true;

      // Build element lookup by ID
      const elementById = new Map<string, ElementDefinition>();
      for (const el of allElements) {
        if (el.data?.id) elementById.set(el.data.id, el);
      }

      // Track which nodes have been revealed (for edge visibility)
      const revealedNodes = new Set<string>();
      let userInteracted = false;

      // Listen for user interaction to suppress auto-fit
      const onViewport = () => { userInteracted = true; };
      cy.on("viewport", onViewport);

      const revealWave = (waveIndex: number) => {
        if (waveIndex >= waves.length || !isRevealingRef.current) {
          isRevealingRef.current = false;
          cy.off("viewport", onViewport);
          return;
        }

        const wave = waves[waveIndex];
        const batch: ElementDefinition[] = [];

        // If beyond maxWaves, batch all remaining
        const nodeIds =
          waveIndex >= options.maxWaves
            ? waves.slice(waveIndex).flat()
            : wave;

        // Add compound parents first if any child references them
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el) continue;
          const parentId = el.data?.parent as string | undefined;
          if (parentId && !revealedNodes.has(parentId)) {
            const parentEl = elementById.get(parentId);
            if (parentEl) {
              batch.push(parentEl);
              revealedNodes.add(parentId);
            }
          }
        }

        // Add nodes with cached positions
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el || el.group === "edges") continue;
          const pos = positions[id];
          if (pos) {
            batch.push({ ...el, position: pos });
          } else {
            batch.push(el);
          }
          revealedNodes.add(id);
        }

        // Add edges where both endpoints are now revealed
        for (const el of allElements) {
          if (el.group !== "edges") continue;
          const src = el.data?.source as string;
          const tgt = el.data?.target as string;
          const edgeId = el.data?.id as string;
          if (
            revealedNodes.has(src) &&
            revealedNodes.has(tgt) &&
            !cy.getElementById(edgeId).nonempty()
          ) {
            batch.push(el);
          }
        }

        if (batch.length > 0) {
          const added = cy.add(batch);
          // Animate reveal: start transparent + small, animate to full
          const childless = added.filter("node:childless");
          // Start at 70% of actual size + transparent, animate to full
          childless.forEach((node) => {
            const size = (node.data("size") as number) || 20;
            node.style({ opacity: 0, width: size * 0.7, height: size * 0.7 });
            node.animate({
              style: { opacity: 1, width: size, height: size },
              duration: 200,
              easing: "ease-out",
            });
          });

          // Auto-fit (only if user hasn't panned/zoomed)
          if (!userInteracted) {
            cy.animate({ fit: { padding: 40 }, duration: 200 });
          }

          // Hub pulse effect on wave 0
          if (waveIndex === 0) {
            const hubNode = childless.first();
            if (hubNode.nonempty()) {
              hubNode.addClass("hub-pulse");
              setTimeout(() => hubNode.removeClass("hub-pulse"), 800);
            }
          }
        }

        // Make compound parents non-interactive
        cy.nodes(":parent").ungrabify().unselectify();

        // Schedule next wave (or finish if we batched remaining)
        if (waveIndex >= options.maxWaves) {
          isRevealingRef.current = false;
          cy.off("viewport", onViewport);
          return;
        }

        animationRef.current = window.setTimeout(
          () => revealWave(waveIndex + 1),
          options.waveDelay,
        );
      };

      revealWave(0);
    },
    [cancelReveal],
  );

  return { revealWithPositions, cancelReveal, isRevealing: isRevealingRef };
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useProgressiveReveal.ts
git commit -m "feat(notes): add progressive BFS reveal hook for graph loading"
```

---

## Task 7: Cola Physics Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useColaPhysics.ts`
- Modify: `desktop-ui/src/features/notes/hooks/useCytoscapeTheme.ts:54-178` (add Cola visual styles)

- [ ] **Step 1: Add Cola visual feedback styles to useCytoscapeTheme**

In `desktop-ui/src/features/notes/hooks/useCytoscapeTheme.ts`, add these stylesheet entries after the `edge.dimmed` style (before the closing `];`):

```ts
      // ── Hub pulse during progressive reveal ──
      {
        selector: "node:childless.hub-pulse",
        style: {
          "border-width": 4,
          "border-color": brand,
          "border-opacity": 0.6,
          "shadow-blur": 20,
          "shadow-color": brand,
          "shadow-opacity": 0.4,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
        },
      },

      // ── Cola drag halo ──
      {
        selector: "node:childless.cola-dragging",
        style: {
          "shadow-blur": 15,
          "shadow-color": brand,
          "shadow-opacity": 0.35,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
          width: (ele: cytoscape.NodeSingular) => (ele.data("size") as number ?? 20) * 1.05,
          height: (ele: cytoscape.NodeSingular) => (ele.data("size") as number ?? 20) * 1.05,
        },
      },

      // ── Cola neighbor glow ──
      {
        selector: "node:childless.cola-neighbor",
        style: {
          "border-width": 2.5,
          "border-opacity": 0.5,
          "border-color": brand,
        },
      },
```

Note: Cytoscape style functions receive `ele` as parameter. The `as` cast is needed for TypeScript. If the function style causes issues, use static values instead:
```ts
      {
        selector: "node:childless.cola-dragging",
        style: {
          "shadow-blur": 15,
          "shadow-color": brand,
          "shadow-opacity": 0.35,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
        },
      },
```

- [ ] **Step 2: Implement useColaPhysics**

Create `desktop-ui/src/features/notes/hooks/useColaPhysics.ts`:

```ts
import type { Core, Layouts } from "cytoscape";
import { useCallback, useEffect, useRef } from "react";
import { snapshotPositions, type PositionMap } from "../lib/graphUtils";
import type { GraphSettings } from "./useGraphSettings";

const SETTLE_DURATION_MS = 300;
const IDLE_TIMEOUT_MS = 30_000;
const HUB_CONNECTION_CAP = 8;

interface UseColaPhysicsParams {
  cy: React.MutableRefObject<Core | null>;
  settings: GraphSettings;
  onPositionsChanged: (positions: PositionMap) => void;
}

function getNeighborhood(cy: Core, nodeId: string, hops: number): Set<string> {
  const visited = new Set<string>();
  let frontier = [nodeId];
  visited.add(nodeId);

  for (let i = 0; i < hops; i++) {
    const nextFrontier: string[] = [];
    for (const id of frontier) {
      const node = cy.getElementById(id);
      if (node.empty()) continue;
      node.neighborhood("node:childless").forEach((n) => {
        if (!visited.has(n.id())) {
          visited.add(n.id());
          nextFrontier.push(n.id());
        }
      });
    }
    frontier = nextFrontier;
  }

  return visited;
}

/**
 * Cola physics hook. Provides:
 * - Auto-activate on drag (scoped to N-hop neighborhood)
 * - Live Physics mode (continuous simulation on visible nodes)
 */
export function useColaPhysics({
  cy: cyRef,
  settings,
  onPositionsChanged,
}: UseColaPhysicsParams) {
  const activeLayoutRef = useRef<Layouts | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const idleTimerRef = useRef<number | null>(null);
  const livePhysicsActiveRef = useRef(false);

  const stopActiveLayout = useCallback(() => {
    if (activeLayoutRef.current) {
      activeLayoutRef.current.stop();
      activeLayoutRef.current = null;
    }
    const cy = cyRef.current;
    if (cy) {
      cy.nodes().unlock();
      cy.nodes(":childless").removeClass("cola-dragging cola-neighbor");
    }
  }, [cyRef]);

  /**
   * Run scoped Cola on drag: lock all nodes except the neighborhood,
   * start Cola with infinite: true, stop after release + settle.
   */
  const startDragCola = useCallback(
    (draggedNodeId: string) => {
      const cy = cyRef.current;
      if (!cy) return;

      stopActiveLayout();

      const totalNodes = cy.nodes(":childless").length;
      const hops = totalNodes >= 800 ? 1 : 2;
      let scope = getNeighborhood(cy, draggedNodeId, hops);

      // Cap hub connections
      if (scope.size > HUB_CONNECTION_CAP + 1) {
        const draggedNode = cy.getElementById(draggedNodeId);
        const neighbors = draggedNode
          .neighborhood("node:childless")
          .sort((a, b) => {
            const wA = a.connectedEdges().reduce((sum, e) => sum + ((e.data("weight") as number) || 1), 0);
            const wB = b.connectedEdges().reduce((sum, e) => sum + ((e.data("weight") as number) || 1), 0);
            return wB - wA;
          })
          .slice(0, HUB_CONNECTION_CAP);

        scope = new Set([draggedNodeId]);
        neighbors.forEach((n) => scope.add(n.id()));
      }

      // Lock all nodes outside scope
      cy.nodes(":childless").forEach((node) => {
        if (!scope.has(node.id())) {
          node.lock();
        }
      });

      // Visual feedback
      cy.getElementById(draggedNodeId).addClass("cola-dragging");
      scope.forEach((id) => {
        if (id !== draggedNodeId) {
          cy.getElementById(id).addClass("cola-neighbor");
        }
      });

      // Start Cola
      const s = settingsRef.current;
      const layout = cy.layout({
        name: "cola",
        infinite: true,
        fit: false,
        animate: true,
        handleDisconnected: false,
        edgeLength: s.linkDistance,
        nodeSpacing: Math.max(5, Math.round(s.repulsion / 1000)),
        unconstrainedIterations: 50,
        userConstraintIterations: 100,
      } as Record<string, unknown>);

      activeLayoutRef.current = layout;
      layout.run();
    },
    [cyRef, stopActiveLayout],
  );

  const stopDragCola = useCallback(() => {
    // Let Cola settle briefly, then stop
    setTimeout(() => {
      stopActiveLayout();
      const cy = cyRef.current;
      if (cy) {
        onPositionsChanged(snapshotPositions(cy));
      }
    }, SETTLE_DURATION_MS);
  }, [cyRef, stopActiveLayout, onPositionsChanged]);

  /**
   * Toggle Live Physics mode: continuous Cola on viewport-visible nodes.
   */
  const startLivePhysics = useCallback(() => {
    const cy = cyRef.current;
    if (!cy) return;

    stopActiveLayout();
    livePhysicsActiveRef.current = true;

    // Lock off-screen nodes
    const extent = cy.extent();
    const buffer = 200; // px buffer around viewport
    cy.nodes(":childless").forEach((node) => {
      const pos = node.position();
      if (
        pos.x < extent.x1 - buffer ||
        pos.x > extent.x2 + buffer ||
        pos.y < extent.y1 - buffer ||
        pos.y > extent.y2 + buffer
      ) {
        node.lock();
      }
    });

    const s = settingsRef.current;
    const layout = cy.layout({
      name: "cola",
      infinite: true,
      fit: false,
      animate: true,
      handleDisconnected: false,
      edgeLength: s.linkDistance,
      nodeSpacing: Math.max(5, Math.round(s.repulsion / 1000)),
    } as Record<string, unknown>);

    activeLayoutRef.current = layout;
    layout.run();

    // Auto-pause after idle
    const resetIdle = () => {
      if (idleTimerRef.current !== null) clearTimeout(idleTimerRef.current);
      idleTimerRef.current = window.setTimeout(() => {
        if (livePhysicsActiveRef.current) {
          stopActiveLayout();
          livePhysicsActiveRef.current = false;
          const c = cyRef.current;
          if (c) onPositionsChanged(snapshotPositions(c));
        }
      }, IDLE_TIMEOUT_MS);
    };

    // Update lock set on viewport pan/zoom (debounced)
    let viewportTimer: number | null = null;
    const updateLockSet = () => {
      if (viewportTimer !== null) clearTimeout(viewportTimer);
      viewportTimer = window.setTimeout(() => {
        const ext = cy.extent();
        const buf = 200;
        cy.nodes(":childless").forEach((node) => {
          const p = node.position();
          const offScreen =
            p.x < ext.x1 - buf || p.x > ext.x2 + buf ||
            p.y < ext.y1 - buf || p.y > ext.y2 + buf;
          if (offScreen && !node.locked()) node.lock();
          else if (!offScreen && node.locked()) node.unlock();
        });
      }, 100);
    };
    cy.on("viewport", updateLockSet);

    cy.on("mousemove", resetIdle);
    resetIdle();
  }, [cyRef, stopActiveLayout, onPositionsChanged]);

  const stopLivePhysics = useCallback(() => {
    livePhysicsActiveRef.current = false;
    stopActiveLayout();
    const cy = cyRef.current;
    if (cy) {
      onPositionsChanged(snapshotPositions(cy));
    }
    if (idleTimerRef.current !== null) {
      clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
  }, [cyRef, stopActiveLayout, onPositionsChanged]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      stopActiveLayout();
      if (idleTimerRef.current !== null) clearTimeout(idleTimerRef.current);
    };
  }, [stopActiveLayout]);

  return {
    startDragCola,
    stopDragCola,
    startLivePhysics,
    stopLivePhysics,
    isLivePhysicsActive: livePhysicsActiveRef,
  };
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useColaPhysics.ts desktop-ui/src/features/notes/hooks/useCytoscapeTheme.ts
git commit -m "feat(notes): add Cola physics hook with drag + live physics modes"
```

---

## Task 8: Refactor useCytoscapeGraph (Core Integration)

This is the main integration task. Replace the current `cy.json({ elements })` approach with element diffing + position cache + progressive reveal + Cola handoff.

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useCytoscapeGraph.ts` (full rewrite)

- [ ] **Step 1: Rewrite useCytoscapeGraph**

Replace the entire contents of `desktop-ui/src/features/notes/hooks/useCytoscapeGraph.ts`:

```ts
import cytoscape, { type Core, type ElementDefinition, type Stylesheet } from "cytoscape";
import { useCallback, useEffect, useRef } from "react";
import { diffElements } from "../lib/elementDiff";
import { registerCytoscapePlugins, snapshotPositions, type PositionMap } from "../lib/graphUtils";
import { useColaPhysics } from "./useColaPhysics";
import type { GraphSettings } from "./useGraphSettings";
import { useProgressiveReveal } from "./useProgressiveReveal";

registerCytoscapePlugins();

interface UseCytoscapeGraphParams {
  containerRef: React.RefObject<HTMLDivElement | null>;
  elements: ElementDefinition[];
  stylesheet: Stylesheet[];
  settings: GraphSettings;
  /** BFS waves for progressive reveal (from graphBfs.ts) */
  waves: string[][];
  /** Cached positions (null = cache miss, undefined = not yet loaded) */
  cachedPositions: PositionMap | null | undefined;
  /** Whether the cache check has completed */
  cacheReady: boolean;
  /** Callback to save positions after layout completes */
  onSavePositions: (positions: PositionMap) => void;
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
  onNodeContext?: (id: string, x: number, y: number) => void;
}

function buildFcoseOptions(settings: GraphSettings, overrides?: Record<string, unknown>) {
  return {
    name: "fcose" as const,
    animate: true,
    animationDuration: 600,
    fit: true,
    padding: 40,
    nodeSeparation: 75,
    idealEdgeLength: settings.linkDistance,
    nodeRepulsion: settings.repulsion,
    edgeElasticity: 0.45,
    gravity: settings.centerForce,
    gravityRange: 1.5,
    nestingFactor: 0.1,
    numIter: 2500,
    quality: "default" as const,
    ...overrides,
  };
}

export function useCytoscapeGraph({
  containerRef,
  elements,
  stylesheet,
  settings,
  waves,
  cachedPositions,
  cacheReady,
  onSavePositions,
  onNodeClick,
  onNodeDoubleClick,
  onNodeHover,
  onNodeContext,
}: UseCytoscapeGraphParams): { cy: React.MutableRefObject<Core | null>; runLayout: () => void } {
  const cyRef = useRef<Core | null>(null);
  const prevElementsRef = useRef<ElementDefinition[]>([]);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const initialLoadDoneRef = useRef(false);

  // Sub-hooks
  const { revealWithPositions, cancelReveal } = useProgressiveReveal();
  const {
    startDragCola,
    stopDragCola,
    startLivePhysics,
    stopLivePhysics,
  } = useColaPhysics({
    cy: cyRef,
    settings,
    onPositionsChanged: onSavePositions,
  });

  // ── Create Cytoscape instance on mount ──
  useEffect(() => {
    if (!containerRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      elements: [], // Start empty — progressive reveal will add elements
      style: stylesheet,
      layout: { name: "preset" },
      minZoom: 0.1,
      maxZoom: 5,
      wheelSensitivity: 0.3,
      boxSelectionEnabled: true,
      selectionType: "single",
      autoungrabify: false,
    });

    cyRef.current = cy;
    initialLoadDoneRef.current = false;

    // ── Node events ──
    cy.on("tap", "node:childless", (evt) => onNodeClick?.(evt.target.id()));
    cy.on("dbltap", "node:childless", (evt) => onNodeDoubleClick?.(evt.target.id()));

    cy.on("mouseover", "node:childless", (evt) => {
      const node = evt.target;
      const pos = node.renderedPosition();
      onNodeHover?.(node.id(), pos.x, pos.y);
      node.addClass("hovered");
      const neighborhood = node.neighborhood().add(node);
      cy.elements().not(neighborhood).addClass("dimmed");
      neighborhood.connectedEdges().addClass("highlighted");
    });

    cy.on("mouseout", "node:childless", () => {
      onNodeHover?.(null, 0, 0);
      cy.elements().removeClass("dimmed").removeClass("highlighted").removeClass("hovered");
    });

    cy.on("cxttap", "node:childless", (evt) => {
      const node = evt.target;
      const pos = node.renderedPosition();
      onNodeContext?.(node.id(), pos.x, pos.y);
    });

    // ── Drag → Cola physics ──
    cy.on("grab", "node:childless", (evt) => {
      startDragCola(evt.target.id());
    });

    cy.on("free", "node:childless", () => {
      stopDragCola();
    });

    // ── Zoom-adaptive labels ──
    cy.on("zoom", () => {
      const zoom = cy.zoom();
      const threshold = settingsRef.current.labelThreshold;
      const childless = cy.nodes(":childless");
      if (zoom < threshold) {
        childless.addClass("hide-label");
      } else {
        childless.removeClass("hide-label");
      }
    });

    // ── Keyboard shortcuts ──
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA"
      )
        return;
      switch (e.key) {
        case "+":
        case "=":
          cy.zoom({
            level: cy.zoom() * 1.2,
            renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 },
          });
          break;
        case "-":
          cy.zoom({
            level: cy.zoom() / 1.2,
            renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 },
          });
          break;
        case "f":
          cy.animate({ fit: { padding: 40 }, duration: 300 });
          break;
        case "Escape":
          cy.elements(":selected").unselect();
          break;
      }
    };
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      cancelReveal();
      document.removeEventListener("keydown", handleKeyDown);
      cy.destroy();
      cyRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount/unmount only
  }, [containerRef, stylesheet]);

  // ── Initial load: progressive reveal or fCoSE ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || elements.length === 0 || initialLoadDoneRef.current || !cacheReady) return;

    initialLoadDoneRef.current = true;
    prevElementsRef.current = elements;

    if (cachedPositions && Object.keys(cachedPositions).length > 0) {
      // Cache HIT → progressive reveal with cached positions
      revealWithPositions(cy, waves, elements, cachedPositions, {
        waveDelay: 80,
        maxWaves: 5,
        instant: settingsRef.current.instantLoad,
      });
    } else if (settingsRef.current.instantLoad) {
      // Cache MISS + instant mode → add all, run fCoSE once
      cy.add(elements);
      cy.nodes(":parent").ungrabify().unselectify();
      const layout = cy.layout(buildFcoseOptions(settingsRef.current));
      layout.on("layoutstop", () => {
        onSavePositions(snapshotPositions(cy));
      });
      layout.run();
    } else {
      // Cache MISS → progressive fCoSE: add nodes wave by wave,
      // pinning earlier waves with fixedNodeConstraint.
      // Build element lookup by ID
      const elementById = new Map<string, ElementDefinition>();
      for (const el of elements) {
        if (el.data?.id) elementById.set(el.data.id, el);
      }
      const allEdges = elements.filter((el) => el.group === "edges");
      const revealedNodes = new Set<string>();
      const maxAnimatedWaves = Math.min(waves.length, 5);
      const totalNodes = elements.filter((el) => el.group !== "edges").length;

      const revealWaveFcose = (waveIndex: number) => {
        if (waveIndex >= waves.length) {
          // All waves done — snapshot final positions
          onSavePositions(snapshotPositions(cy));
          return;
        }

        // Batch remaining waves if beyond max
        const nodeIds = waveIndex >= maxAnimatedWaves
          ? waves.slice(waveIndex).flat()
          : waves[waveIndex];

        const batch: ElementDefinition[] = [];

        // Add compound parents first
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el) continue;
          const parentId = el.data?.parent as string | undefined;
          if (parentId && !revealedNodes.has(parentId)) {
            const parentEl = elementById.get(parentId);
            if (parentEl) {
              batch.push(parentEl);
              revealedNodes.add(parentId);
            }
          }
        }

        // Add nodes
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el || el.group === "edges") continue;
          batch.push(el);
          revealedNodes.add(id);
        }

        // Add edges where both endpoints visible
        for (const el of allEdges) {
          const src = el.data?.source as string;
          const tgt = el.data?.target as string;
          const edgeId = el.data?.id as string;
          if (revealedNodes.has(src) && revealedNodes.has(tgt) && !cy.getElementById(edgeId).nonempty()) {
            batch.push(el);
          }
        }

        if (batch.length > 0) cy.add(batch);
        cy.nodes(":parent").ungrabify().unselectify();

        // Build fixedNodeConstraint for all previously placed nodes
        const fixedConstraints: { nodeId: string; position: { x: number; y: number } }[] = [];
        cy.nodes(":childless").forEach((n) => {
          if (!nodeIds.includes(n.id())) {
            const pos = n.position();
            fixedConstraints.push({ nodeId: n.id(), position: { x: pos.x, y: pos.y } });
          }
        });

        // Choose iteration count based on wave index
        const numIter = waveIndex <= 2 ? 2500 : waveIndex <= 4 ? 1500 : 1000;

        const layout = cy.layout(buildFcoseOptions(settingsRef.current, {
          fit: true,
          animate: true,
          animationDuration: 400,
          randomize: false,
          quality: "proof",
          numIter,
          fixedNodeConstraint: fixedConstraints.length > 0 ? fixedConstraints : undefined,
        }));

        layout.on("layoutstop", () => {
          const isFinal = waveIndex >= maxAnimatedWaves || waveIndex >= waves.length - 1;
          if (isFinal) {
            onSavePositions(snapshotPositions(cy));
          } else {
            setTimeout(() => revealWaveFcose(waveIndex + 1), 150);
          }
        });

        layout.run();
      };

      revealWaveFcose(0);
    }
  }, [elements, cachedPositions, cacheReady, waves, revealWithPositions, onSavePositions, cancelReveal]);

  // ── Incremental updates: element diffing ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !initialLoadDoneRef.current) return;

    // Capture old elements BEFORE updating the ref (fixes stale ref bug)
    const prevElements = prevElementsRef.current;
    const diff = diffElements(prevElements, elements);
    if (!diff.hasChanges) return;

    prevElementsRef.current = elements;

    // Remove first (prevents dangling edges)
    for (const id of diff.removedEdgeIds) {
      cy.getElementById(id).remove();
    }
    for (const id of diff.removedNodeIds) {
      cy.getElementById(id).remove();
    }

    // Add compound parents before their children
    const parentNodes = diff.addedNodes.filter((el) => !el.data?.parent && el.data?.type);
    const childNodes = diff.addedNodes.filter((el) => el.data?.parent || !el.data?.type);

    if (parentNodes.length > 0) cy.add(parentNodes);

    if (childNodes.length > 0) {
      // Place new nodes near their neighbors if possible
      for (const el of childNodes) {
        const id = el.data?.id;
        if (!id) continue;
        // Find connected nodes already in the graph
        const connectedEdges = elements.filter(
          (e) =>
            e.group === "edges" &&
            (e.data?.source === id || e.data?.target === id),
        );
        const neighborIds = connectedEdges
          .map((e) =>
            e.data?.source === id ? e.data?.target : e.data?.source,
          )
          .filter((nid): nid is string => !!nid && cy.getElementById(nid as string).nonempty());

        if (neighborIds.length > 0) {
          let avgX = 0;
          let avgY = 0;
          for (const nid of neighborIds) {
            const pos = cy.getElementById(nid).position();
            avgX += pos.x;
            avgY += pos.y;
          }
          avgX /= neighborIds.length;
          avgY /= neighborIds.length;
          const angle = Math.random() * Math.PI * 2;
          const offset = 120 + Math.random() * 60;
          el.position = {
            x: avgX + Math.cos(angle) * offset,
            y: avgY + Math.sin(angle) * offset,
          };
        }
      }

      cy.add(childNodes);
    }

    if (diff.addedEdges.length > 0) {
      cy.add(diff.addedEdges);
    }

    cy.nodes(":parent").ungrabify().unselectify();

    // Run scoped fCoSE for new nodes only (existing stay pinned)
    if (childNodes.length > 0) {
      // Use prevElements (captured before ref update) to identify existing nodes
      const existingNodeIds = new Set(
        prevElements
          .filter((el) => el.group !== "edges")
          .map((el) => el.data?.id)
          .filter(Boolean) as string[],
      );
      const fixedConstraints = cy
        .nodes(":childless")
        .filter((n) => existingNodeIds.has(n.id()) || !childNodes.some((el) => el.data?.id === n.id()))
        .map((n) => ({
          nodeId: n.id(),
          position: { x: n.position().x, y: n.position().y },
        }));

      if (fixedConstraints.length > 0) {
        const layout = cy.layout(
          buildFcoseOptions(settingsRef.current, {
            fit: false,
            animate: true,
            animationDuration: 400,
            randomize: false,
            quality: "proof",
            fixedNodeConstraint: fixedConstraints,
          }),
        );
        layout.on("layoutstop", () => {
          onSavePositions(snapshotPositions(cy));
        });
        layout.run();
      }
    } else if (diff.removedNodeIds.length > 0) {
      // Just save updated positions after removal
      onSavePositions(snapshotPositions(cy));
    }
  }, [elements, onSavePositions]);

  // ── Re-layout when physics settings change ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || cy.elements().length === 0 || !initialLoadDoneRef.current) return;
    const layout = cy.layout(buildFcoseOptions(settings));
    layout.on("layoutstop", () => {
      onSavePositions(snapshotPositions(cy));
    });
    layout.run();
  }, [settings.linkDistance, settings.repulsion, settings.centerForce, onSavePositions]);

  // ── Update node sizes when nodeScale changes ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.nodes(":childless").forEach((node) => {
      const baseSize = node.data("size") as number;
      if (baseSize) {
        const scaled = baseSize * settings.nodeScale;
        node.style({ width: scaled, height: scaled });
      }
    });
  }, [settings.nodeScale]);

  // ── Update arrow visibility ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.edges().style({
      "target-arrow-shape": settings.showArrows ? "triangle" : "none",
    });
  }, [settings.showArrows]);

  // ── Live Physics toggle ──
  useEffect(() => {
    if (settings.livePhysics) {
      startLivePhysics();
    } else {
      stopLivePhysics();
    }
  }, [settings.livePhysics, startLivePhysics, stopLivePhysics]);

  const runLayout = useCallback(() => {
    const cy = cyRef.current;
    if (!cy) return;
    const layout = cy.layout(buildFcoseOptions(settingsRef.current));
    layout.on("layoutstop", () => {
      onSavePositions(snapshotPositions(cy));
    });
    layout.run();
  }, [onSavePositions]);

  return { cy: cyRef, runLayout };
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

Fix any type errors that arise. Common issues:
- `elements` type mismatch from the `position` field added inline
- `layout.on` typing for fCoSE events

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useCytoscapeGraph.ts
git commit -m "refactor(notes): replace cy.json() with element diffing + position cache + Cola handoff"
```

---

## Task 9: Wire Everything in GraphView

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx:1-281`

- [ ] **Step 1: Update GraphView to wire all new hooks**

In `desktop-ui/src/features/notes/components/GraphView.tsx`, make these changes:

1. **Add imports** at the top:
```ts
import { Activity } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { computeBfsWaves, selectHub } from "../lib/graphBfs";
import type { PositionMap } from "../lib/graphUtils";
import { useGraphPositionCache } from "../hooks/useGraphPositionCache";
```

2. **Add position cache + BFS hooks** after `useCytoscapeElements`:

```ts
  // Position cache — undefined = not yet loaded, null = cache miss, PositionMap = cache hit
  const { loadPositions, savePositions } = useGraphPositionCache(smartView, fingerprint);
  const [cachedPositions, setCachedPositions] = useState<PositionMap | null | undefined>(undefined);
  const [cacheReady, setCacheReady] = useState(false);

  // Load positions on mount or fingerprint change
  useEffect(() => {
    setCacheReady(false);
    setCachedPositions(undefined);
    loadPositions().then((pos) => {
      setCachedPositions(pos); // null if cache miss, PositionMap if hit
      setCacheReady(true);
    });
  }, [loadPositions, fingerprint]);

  // BFS waves
  const waves = useMemo(() => {
    if (filteredNodes.length === 0) return [];
    const adjacency = new Map<string, Set<string>>();
    for (const link of filteredLinks) {
      const sId = typeof link.source === "string" ? link.source : link.source.id;
      const tId = typeof link.target === "string" ? link.target : link.target.id;
      if (!adjacency.has(sId)) adjacency.set(sId, new Set());
      if (!adjacency.has(tId)) adjacency.set(tId, new Set());
      adjacency.get(sId)!.add(tId);
      adjacency.get(tId)!.add(sId);
    }
    const hubId = selectHub(
      filteredNodes.map((n) => ({ id: n.id, linkCount: n.linkCount, title: n.title })),
      activeNoteId,
    );
    return computeBfsWaves(hubId, adjacency, new Set(filteredNodes.map((n) => n.id)));
  }, [filteredNodes, filteredLinks, activeNoteId]);

  const handleSavePositions = useCallback(
    (positions: PositionMap) => {
      savePositions(positions);
    },
    [savePositions],
  );
```

Add `useMemo` to the React import line.

3. **Update `useCytoscapeGraph` call** — pass the new props:

```ts
  const { cy, runLayout } = useCytoscapeGraph({
    containerRef,
    elements,
    stylesheet,
    settings,
    waves,
    cachedPositions,
    cacheReady,
    onSavePositions: handleSavePositions,
    onNodeClick: onSelectNote,
    onNodeDoubleClick: onOpenInEditor,
    onNodeHover: useCallback((id: string | null, x: number, y: number) => {
      if (id) {
        setTooltip({ nodeId: id, x, y });
      } else {
        setTooltip(null);
      }
    }, []),
  });
```

4. **Don't render graph until cache check completes** — wrap the graph container:

```tsx
{!cacheReady ? (
  <div className="flex-1 flex items-center justify-center text-muted text-sm">
    Loading graph...
  </div>
) : (
  <div
    className="flex-1 relative min-h-0 bg-background"
    style={{
      backgroundImage: "radial-gradient(circle, var(--border) 0.5px, transparent 0.5px)",
      backgroundSize: "20px 20px",
    }}
  >
    {/* ... existing graph content ... */}
  </div>
)}
```

5. **Add Live Physics toggle button** in the controls section (before the zoom buttons):

```tsx
          <button
            type="button"
            onClick={() => setSettings({ livePhysics: !settings.livePhysics })}
            className={`w-7 h-7 glass-button flex items-center justify-center transition-colors ${
              settings.livePhysics ? "text-brand" : "text-secondary hover:text-primary"
            }`}
            aria-label="Live physics"
            title={settings.livePhysics ? "Disable live physics" : "Enable live physics"}
          >
            <Activity size={14} />
          </button>
```

6. **Destructure `fingerprint`** from `useCytoscapeElements`:

```ts
  const { elements: allElements, clusters, fingerprint } = useCytoscapeElements({
```

- [ ] **Step 2: Verify it compiles**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

- [ ] **Step 3: Verify the full build works**

```bash
cd desktop-ui && bun run build
```
Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(notes): wire progressive reveal + position cache + Cola physics into GraphView"
```

---

## Task 10: Manual Testing & Polish

- [ ] **Step 1: Run the dev app**

```bash
cargo tauri dev
```

Open the Knowledge Base page. Verify:
- Graph loads with progressive wave reveal (nodes appear from center outward)
- Reopening the graph loads instantly from cache (fast wave animation)
- Dragging a node causes neighbors to follow (Cola physics)
- Adding a link between notes does NOT rearrange the entire graph
- Settings sliders trigger full re-layout (expected behavior)
- Live Physics toggle makes the graph "breathe"
- "Instant Load" toggle skips the wave animation

- [ ] **Step 2: Run linter**

```bash
cd desktop-ui && bun run lint:fix
```

- [ ] **Step 3: Run all tests**

```bash
cd desktop-ui && bun run test
```
Expected: All tests pass (graphFingerprint, graphBfs, elementDiff, plus existing tests).

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "style(notes): lint fixes for graph progressive physics"
```

---

## Verification Checklist

Before marking complete, verify each behavior:

- [ ] **Progressive reveal (cache miss):** First load shows nodes expanding from hub outward in waves
- [ ] **Progressive reveal (cache hit):** Reopen shows fast staggered fade-in from cached positions
- [ ] **Instant load setting:** Toggling "Instant Load" in settings skips wave animation
- [ ] **Position persistence:** Drag a node, reopen the graph — node stays where you put it
- [ ] **Localized updates:** Edit a note's content → graph doesn't move. Add a link → only endpoints adjust.
- [ ] **Cola drag physics:** Grab a node → neighbors follow. Release → neighbors settle naturally.
- [ ] **Live Physics:** Toggle on → graph breathes. Toggle off → positions saved to cache.
- [ ] **Settings re-layout:** Changing repulsion/distance/center triggers full fCoSE re-layout and saves new positions
- [ ] **View mode switching:** Each view (Full/Local/By Tag/etc.) has independent positions
- [ ] **No regressions:** Hover tooltip, cluster legend, zoom controls, keyboard shortcuts all still work
