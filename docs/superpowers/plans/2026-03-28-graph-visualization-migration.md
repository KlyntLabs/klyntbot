# Graph Visualization Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Cytoscape.js graph rendering with react-force-graph (2D + 3D hybrid) featuring custom glow painting, progressive BFS reveal, and a viewport-rectangle minimap.

**Architecture:** Full replacement — rip out Cytoscape, build new 2D canvas renderer (react-force-graph-2d) with custom painters, add 3D "Brain View" toggle (react-force-graph-3d + Three.js bloom). Flat node/link data model with color+proximity clustering via custom d3-force. Reuse existing BFS, fingerprint, position cache, and data hooks.

**Tech Stack:** react-force-graph-2d, react-force-graph-3d, three, d3-force (bundled), TypeScript, React, Tailwind v4, Canvas API, Three.js post-processing

**Spec:** `docs/superpowers/specs/2026-03-28-graph-visualization-migration-design.md`

---

### Task 1: Install new dependencies and remove old ones

**Files:**
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Install new packages**

Run:
```bash
cd desktop-ui && bun add react-force-graph-2d react-force-graph-3d three && bun add -d @types/three
```

- [ ] **Step 2: Remove Cytoscape packages**

Run:
```bash
cd desktop-ui && bun remove cytoscape cytoscape-cola cytoscape-fcose @types/cytoscape
```

Note: Keep `d3-force` and `@types/d3-force` — `ScopePreview.tsx` still uses them.

- [ ] **Step 3: Verify install**

Run:
```bash
cd desktop-ui && bun run build
```

Expected: Build succeeds (Cytoscape imports will fail in graph files, but those files aren't the entry point — Vite tree-shakes). If build fails due to missing Cytoscape imports, that's fine — we'll delete those files in Task 15.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lockb
git commit -m "feat(graph): add react-force-graph + three.js, remove cytoscape dependencies"
```

---

### Task 2: Update GraphSettings with new fields

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useGraphSettings.ts`

- [ ] **Step 1: Update the GraphSettings interface and defaults**

Replace the full content of `desktop-ui/src/features/notes/hooks/useGraphSettings.ts`:

```typescript
import { useCallback, useState } from "react";

export interface GraphSettings {
  /** Link distance between connected nodes (px) */
  linkDistance: number;
  /** Node repulsion strength (higher = more spread) */
  repulsion: number;
  /** Center gravity (higher = tighter cluster) */
  centerForce: number;
  /** Node size multiplier (1 = default) */
  nodeScale: number;
  /** Zoom level below which labels hide */
  labelThreshold: number;
  /** Show directional arrows on edges */
  showArrows: boolean;
  /** Show orphan (unlinked) nodes */
  showOrphans: boolean;
  /** Enable continuous physics simulation */
  livePhysics: boolean;
  /** Render mode: 2D canvas or 3D WebGL */
  renderMode: "2d" | "3d";
  /** Progressive reveal speed */
  revealSpeed: "instant" | "balanced" | "cinematic";
  /** Clustering mode for node grouping */
  clusteringMode: "notebook" | "semantic";
  /** Auto-rotate in 3D mode when idle */
  idleRotation: boolean;
  /** Show the viewport minimap */
  showMinimap: boolean;
}

const DEFAULT_SETTINGS: GraphSettings = {
  linkDistance: 120,
  repulsion: 8000,
  centerForce: 0.2,
  nodeScale: 1,
  labelThreshold: 0.5,
  showArrows: true,
  showOrphans: true,
  livePhysics: false,
  renderMode: "2d",
  revealSpeed: "balanced",
  clusteringMode: "notebook",
  idleRotation: true,
  showMinimap: true,
};

const STORAGE_KEY = "klynt-graph-settings";

function loadSettings(): GraphSettings {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return { ...DEFAULT_SETTINGS, ...JSON.parse(stored) };
  } catch {
    // ignore
  }
  return DEFAULT_SETTINGS;
}

function saveSettings(settings: GraphSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // ignore
  }
}

export function useGraphSettings() {
  const [settings, setSettingsState] = useState<GraphSettings>(loadSettings);

  const setSettings = useCallback((partial: Partial<GraphSettings>) => {
    setSettingsState((prev) => {
      const next = { ...prev, ...partial };
      saveSettings(next);
      return next;
    });
  }, []);

  const resetSettings = useCallback(() => {
    setSettingsState(DEFAULT_SETTINGS);
    saveSettings(DEFAULT_SETTINGS);
  }, []);

  return { settings, setSettings, resetSettings, defaults: DEFAULT_SETTINGS };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphSettings.ts
git commit -m "feat(graph): extend GraphSettings with renderMode, revealSpeed, clusteringMode, minimap"
```

---

### Task 3: Create useGraphElements hook (replaces useCytoscapeElements)

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useGraphElements.ts`

- [ ] **Step 1: Create the new hook**

Create `desktop-ui/src/features/notes/hooks/useGraphElements.ts`:

```typescript
import type { Notebook } from "@shared/types";
import { useMemo } from "react";
import { computeFingerprint } from "../lib/graphFingerprint";
import type { GraphLink, GraphNode } from "./useGraphData";

const CLUSTER_PALETTE = [
  "#a78bfa",
  "#93c5fd",
  "#6ee7b7",
  "#fcd34d",
  "#fca5a5",
  "#f9a8d4",
  "#a5b4fc",
  "#67e8f9",
  "#fdba74",
  "#86efac",
  "#c4b5fd",
  "#fde68a",
];

export interface ClusterInfo {
  id: string;
  label: string;
  color: string;
  count: number;
}

export interface ForceNode {
  id: string;
  label: string;
  color: string;
  size: number;
  linkCount: number;
  tags: string[];
  bodyPreview: string;
  notebookId: string | null;
  clusterId: string;
  // d3-force managed (populated after simulation)
  x?: number;
  y?: number;
  z?: number;
  fx?: number | null;
  fy?: number | null;
}

export interface ForceLink {
  source: string;
  target: string;
  weight: number;
  color: string;
}

export interface GraphElements {
  nodes: ForceNode[];
  links: ForceLink[];
  clusters: ClusterInfo[];
  fingerprint: string;
}

function getNodeSize(linkCount: number): number {
  const normalized = Math.min(linkCount, 20) / 20;
  return 18 + normalized * 28; // 18px (orphan) → 46px (hub)
}

interface UseGraphElementsParams {
  nodes: GraphNode[];
  links: GraphLink[];
  notebooks: Notebook[];
  clusteringMode: "notebook" | "semantic";
  activeNoteId: string | null;
}

export function useGraphElements({
  nodes,
  links,
  notebooks,
  clusteringMode,
  activeNoteId: _activeNoteId,
}: UseGraphElementsParams): GraphElements {
  return useMemo(() => {
    const clusterMap = new Map<string, ClusterInfo>();
    const notebookMap = new Map<string, Notebook>();
    for (const nb of notebooks) notebookMap.set(nb.id, nb);

    let colorIndex = 0;
    const getClusterColor = (id: string, notebook?: Notebook): string => {
      if (notebook?.color) return notebook.color;
      const existing = clusterMap.get(id);
      if (existing) return existing.color;
      return CLUSTER_PALETTE[colorIndex++ % CLUSTER_PALETTE.length];
    };

    // Build cluster assignments
    const nodeClusterMap = new Map<string, string>();
    // Both 'notebook' and 'semantic' (stubbed) use notebook-based clustering for now
    const hasLinks = new Set<string>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      hasLinks.add(sourceId);
      hasLinks.add(targetId);
    }
    for (const node of nodes) {
      if (node.notebookId) {
        nodeClusterMap.set(node.id, `nb:${node.notebookId}`);
      } else if (hasLinks.has(node.id)) {
        nodeClusterMap.set(node.id, "_floating");
      } else {
        nodeClusterMap.set(node.id, "_isolated");
      }
    }

    // Build cluster info entries
    const seenClusters = new Set<string>();
    for (const [, clusterId] of nodeClusterMap) {
      if (seenClusters.has(clusterId)) continue;
      seenClusters.add(clusterId);

      let label: string;
      let color: string;

      if (clusterId === "_floating") {
        label = "Floating Ideas";
        color = "#9CA3AF";
      } else if (clusterId === "_isolated") {
        label = "Isolated Notes";
        color = "#6B7280";
      } else {
        const nbId = clusterId.replace("nb:", "");
        const nb = notebookMap.get(nbId);
        label = nb?.title || "Unknown Notebook";
        color = getClusterColor(clusterId, nb);
      }

      clusterMap.set(clusterId, { id: clusterId, label, color, count: 0 });
    }

    // Build flat node array
    const forceNodes: ForceNode[] = [];
    for (const node of nodes) {
      const clusterId = nodeClusterMap.get(node.id) || "_isolated";
      const cluster = clusterMap.get(clusterId);
      if (cluster) cluster.count++;

      const color = cluster?.color || "#6B7280";
      const size = getNodeSize(node.linkCount);

      forceNodes.push({
        id: node.id,
        label: node.title,
        color,
        size,
        linkCount: node.linkCount,
        tags: node.tags,
        bodyPreview: node.bodyPreview,
        notebookId: node.notebookId,
        clusterId,
      });
    }

    // Build deduplicated link array
    const edgeCounts = new Map<string, number>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");
      edgeCounts.set(key, (edgeCounts.get(key) || 0) + 1);
    }

    const forceLinks: ForceLink[] = [];
    const seenEdges = new Set<string>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");
      if (seenEdges.has(key)) continue;
      seenEdges.add(key);

      const count = edgeCounts.get(key) || 1;
      const weight = count === 1 ? 1 : count === 2 ? 1.8 : 2.8;
      const sourceCluster = nodeClusterMap.get(sourceId);
      const sourceColor = clusterMap.get(sourceCluster || "")?.color || "#6B7280";

      forceLinks.push({ source: sourceId, target: targetId, weight, color: sourceColor });
    }

    const clusters = Array.from(clusterMap.values()).filter((c) => c.count > 0);

    const nodeIdList = nodes.map((n) => n.id);
    const edgePairList: [string, string][] = forceLinks.map((l) => [l.source, l.target]);
    const fingerprint = computeFingerprint(nodeIdList, edgePairList);

    return { nodes: forceNodes, links: forceLinks, clusters, fingerprint };
  }, [nodes, links, notebooks, clusteringMode]);
}
```

- [ ] **Step 2: Verify no TypeScript errors in the new file**

Run:
```bash
cd desktop-ui && npx tsc --noEmit --pretty 2>&1 | head -20
```

Expected: The new file compiles cleanly. Other files may still have Cytoscape errors (expected until Task 15).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphElements.ts
git commit -m "feat(graph): add useGraphElements hook with flat ForceNode/ForceLink data model"
```

---

### Task 4: Create Canvas paint functions (graphPainters.ts)

**Files:**
- Create: `desktop-ui/src/features/notes/lib/graphPainters.ts`

- [ ] **Step 1: Create the painters module**

Create `desktop-ui/src/features/notes/lib/graphPainters.ts`:

```typescript
import type { ForceNode, ForceLink } from "../hooks/useGraphElements";

/** Hex color → rgba string */
function hexToRgba(hex: string, alpha: number): string {
  const r = Number.parseInt(hex.slice(1, 3), 16);
  const g = Number.parseInt(hex.slice(3, 5), 16);
  const b = Number.parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

export interface PaintContext {
  nodeScale: number;
  labelThreshold: number;
  hoveredNodeId: string | null;
  neighborSet: Set<string>;
  highlightedClusterId: string | null;
}

/**
 * Custom Canvas node painter for react-force-graph-2d.
 * Draws a filled circle with a soft glow ring + optional label.
 */
export function paintNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  paintCtx: PaintContext,
): void {
  const x = node.x ?? 0;
  const y = node.y ?? 0;
  const radius = (node.size / 2) * paintCtx.nodeScale;

  // Determine opacity based on hover/highlight state
  let opacity = 0.85;
  if (paintCtx.hoveredNodeId) {
    if (node.id === paintCtx.hoveredNodeId) {
      opacity = 1;
    } else if (paintCtx.neighborSet.has(node.id)) {
      opacity = 0.85;
    } else {
      opacity = 0.12;
    }
  } else if (paintCtx.highlightedClusterId) {
    opacity = node.clusterId === paintCtx.highlightedClusterId ? 0.9 : 0.12;
  }

  // Glow ring (drawn first, behind the node)
  const prevComposite = ctx.globalCompositeOperation;
  ctx.globalCompositeOperation = "screen";
  const glowRadius = radius * 2.5;
  const gradient = ctx.createRadialGradient(x, y, radius * 0.5, x, y, glowRadius);
  gradient.addColorStop(0, hexToRgba(node.color, 0.2 * opacity));
  gradient.addColorStop(1, hexToRgba(node.color, 0));
  ctx.fillStyle = gradient;
  ctx.beginPath();
  ctx.arc(x, y, glowRadius, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalCompositeOperation = prevComposite;

  // Node circle
  ctx.fillStyle = hexToRgba(node.color, opacity);
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.fill();

  // Border
  ctx.strokeStyle = hexToRgba(node.color, opacity * 0.3);
  ctx.lineWidth = node.id === paintCtx.hoveredNodeId ? 3 : 2;
  ctx.stroke();

  // Label (zoom-adaptive)
  if (globalScale > paintCtx.labelThreshold) {
    const fontSize = Math.max(10 / globalScale, 3);
    ctx.font = `500 ${fontSize}px Inter, system-ui, sans-serif`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle =
      node.id === paintCtx.hoveredNodeId
        ? `rgba(255,255,255,${opacity})`
        : `rgba(255,255,255,${opacity * 0.7})`;
    ctx.fillText(node.label, x + radius + 4 / globalScale, y);
  }
}

/**
 * Custom Canvas link painter for react-force-graph-2d.
 * Draws a subtle gradient line.
 */
export function paintLink(
  link: ForceLink,
  ctx: CanvasRenderingContext2D,
  _globalScale: number,
  paintCtx: PaintContext,
): void {
  const source = link.source as unknown as ForceNode;
  const target = link.target as unknown as ForceNode;
  if (!source.x || !source.y || !target.x || !target.y) return;

  let opacity = 0.35 * link.weight;
  if (paintCtx.hoveredNodeId) {
    const isConnected =
      source.id === paintCtx.hoveredNodeId || target.id === paintCtx.hoveredNodeId;
    opacity = isConnected ? 0.7 * link.weight : 0.05;
  } else if (paintCtx.highlightedClusterId) {
    const isInCluster =
      source.clusterId === paintCtx.highlightedClusterId ||
      target.clusterId === paintCtx.highlightedClusterId;
    opacity = isInCluster ? 0.5 * link.weight : 0.05;
  }

  ctx.strokeStyle = hexToRgba(link.color, Math.min(opacity, 1));
  ctx.lineWidth = Math.max(0.5, link.weight * 0.8);
  ctx.beginPath();
  ctx.moveTo(source.x, source.y);
  ctx.lineTo(target.x, target.y);
  ctx.stroke();
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/lib/graphPainters.ts
git commit -m "feat(graph): add Canvas paint functions with glow ring and hover dimming"
```

---

### Task 5: Create useWaveReveal hook (progressive BFS reveal)

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useWaveReveal.ts`

- [ ] **Step 1: Create the wave reveal controller**

Create `desktop-ui/src/features/notes/hooks/useWaveReveal.ts`:

```typescript
import { useCallback, useRef, useState } from "react";
import { computeBfsWaves, selectHub } from "../lib/graphBfs";
import type { GraphElements, ForceNode } from "./useGraphElements";
import type { PositionMap } from "./useGraphPositionCache";

const WAVE_DELAYS = {
  instant: 0,
  balanced: 80,
  cinematic: 150,
} as const;

const MAX_ANIMATED_WAVES = 5;

export interface WaveRevealController {
  /** Start BFS progressive reveal from hub node */
  revealWave: (
    hubId: string,
    elements: GraphElements,
    cachedPositions?: PositionMap | null,
    waveOrder?: string[][],
  ) => void;
  /** Trigger a micro-reveal pulse on specific nodes (for future cognitive integration) */
  triggerMicroReveal: (nodeIds: string[]) => void;
  /** Progress of current reveal (0–1) */
  revealProgress: number;
  /** Whether a reveal is currently in progress */
  isRevealing: boolean;
  /** Cancel any in-progress reveal */
  cancelReveal: () => void;
  /** Set of node IDs that have been revealed so far */
  revealedNodes: Set<string>;
}

type RevealSpeed = "instant" | "balanced" | "cinematic";

export function useWaveReveal(revealSpeed: RevealSpeed): WaveRevealController {
  const [revealProgress, setRevealProgress] = useState(1);
  const [isRevealing, setIsRevealing] = useState(false);
  const revealedNodesRef = useRef<Set<string>>(new Set());
  const timersRef = useRef<ReturnType<typeof setTimeout>[]>([]);
  const microRevealTimersRef = useRef<ReturnType<typeof setTimeout>[]>([]);

  const cancelReveal = useCallback(() => {
    for (const t of timersRef.current) clearTimeout(t);
    timersRef.current = [];
    setIsRevealing(false);
    setRevealProgress(1);
  }, []);

  const revealWave = useCallback(
    (
      hubId: string,
      elements: GraphElements,
      cachedPositions?: PositionMap | null,
      waveOrder?: string[][],
    ) => {
      // Cancel any existing reveal
      cancelReveal();

      const allNodeIds = new Set(elements.nodes.map((n) => n.id));
      if (allNodeIds.size === 0) return;

      // Compute BFS waves if not provided
      const waves =
        waveOrder ??
        (() => {
          const adjacency = new Map<string, Set<string>>();
          for (const link of elements.links) {
            const sId = typeof link.source === "string" ? link.source : (link.source as unknown as ForceNode).id;
            const tId = typeof link.target === "string" ? link.target : (link.target as unknown as ForceNode).id;
            if (!adjacency.has(sId)) adjacency.set(sId, new Set());
            if (!adjacency.has(tId)) adjacency.set(tId, new Set());
            adjacency.get(sId)?.add(tId);
            adjacency.get(tId)?.add(sId);
          }
          return computeBfsWaves(hubId, adjacency, allNodeIds);
        })();

      if (waves.length === 0) return;

      const totalNodes = allNodeIds.size;
      const delay = WAVE_DELAYS[revealSpeed];

      // Instant mode — reveal everything immediately
      if (delay === 0) {
        revealedNodesRef.current = new Set(allNodeIds);
        setRevealProgress(1);
        setIsRevealing(false);
        return;
      }

      // Progressive reveal
      setIsRevealing(true);
      setRevealProgress(0);
      revealedNodesRef.current = new Set();
      let revealedCount = 0;

      const revealNextWave = (waveIndex: number) => {
        if (waveIndex >= waves.length) {
          setIsRevealing(false);
          setRevealProgress(1);
          return;
        }

        // After MAX_ANIMATED_WAVES, batch remaining
        const nodeIds =
          waveIndex >= MAX_ANIMATED_WAVES ? waves.slice(waveIndex).flat() : waves[waveIndex];

        for (const id of nodeIds) {
          revealedNodesRef.current.add(id);
        }
        revealedCount += nodeIds.length;
        setRevealProgress(Math.min(revealedCount / totalNodes, 1));

        const isFinal = waveIndex >= MAX_ANIMATED_WAVES || waveIndex >= waves.length - 1;
        if (isFinal) {
          // Reveal any remaining
          for (const id of allNodeIds) revealedNodesRef.current.add(id);
          setRevealProgress(1);
          setIsRevealing(false);
        } else {
          const timer = setTimeout(() => revealNextWave(waveIndex + 1), delay);
          timersRef.current.push(timer);
        }
      };

      revealNextWave(0);
    },
    [revealSpeed, cancelReveal],
  );

  const triggerMicroReveal = useCallback((nodeIds: string[]) => {
    // Clear previous micro-reveal timers
    for (const t of microRevealTimersRef.current) clearTimeout(t);
    microRevealTimersRef.current = [];

    // Ensure nodes are in the revealed set
    for (const id of nodeIds) {
      revealedNodesRef.current.add(id);
    }
    // Micro-reveal is purely visual — the paint function reads revealedNodes
    // and applies a pulse effect for recently added nodes.
    // Future: connect to triggerMicroReveal visual effect in paint functions
  }, []);

  return {
    revealWave,
    triggerMicroReveal,
    revealProgress,
    isRevealing,
    cancelReveal,
    revealedNodes: revealedNodesRef.current,
  };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useWaveReveal.ts
git commit -m "feat(graph): add useWaveReveal hook with BFS progressive reveal controller"
```

---

### Task 6: Create useForceGraph hook (2D renderer)

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useForceGraph.ts`

- [ ] **Step 1: Create the 2D force graph hook**

Create `desktop-ui/src/features/notes/hooks/useForceGraph.ts`:

```typescript
import ForceGraph2D, { type ForceGraphMethods } from "react-force-graph-2d";
import {
  forceCollide,
  forceManyBody,
  forceCenter,
  forceX,
  forceY,
  type Simulation,
  type SimulationNodeDatum,
} from "d3-force";
import { useCallback, useEffect, useRef } from "react";
import { paintNode, paintLink, type PaintContext } from "../lib/graphPainters";
import type { ForceNode, ForceLink, GraphElements } from "./useGraphElements";
import type { GraphSettings } from "./useGraphSettings";
import type { PositionMap } from "./useGraphPositionCache";

export { ForceGraph2D };

export interface ViewportBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface GraphNudge {
  nodeId: string;
  position: { x: number; y: number };
  clusterId: string;
  timestamp: number;
}

interface UseForceGraphParams {
  containerRef: React.RefObject<HTMLDivElement | null>;
  elements: GraphElements;
  settings: GraphSettings;
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
  onNodeNudge?: (nudge: GraphNudge) => void;
  onPositionsChanged?: (positions: PositionMap) => void;
  highlightedClusterId: string | null;
  revealedNodes: Set<string>;
}

function snapshotPositions(nodes: ForceNode[]): PositionMap {
  const positions: PositionMap = {};
  for (const node of nodes) {
    if (node.x != null && node.y != null) {
      positions[node.id] = { x: node.x, y: node.y };
    }
  }
  return positions;
}

/** Custom cluster attraction force — pulls same-clusterId nodes together */
function forceCluster(nodes: ForceNode[], strength = 0.3) {
  const centroids = new Map<string, { x: number; y: number; count: number }>();

  function force(alpha: number) {
    // Compute cluster centroids
    centroids.clear();
    for (const node of nodes) {
      if (node.x == null || node.y == null) continue;
      const c = centroids.get(node.clusterId);
      if (c) {
        c.x += node.x;
        c.y += node.y;
        c.count++;
      } else {
        centroids.set(node.clusterId, { x: node.x, y: node.y, count: 1 });
      }
    }
    for (const c of centroids.values()) {
      c.x /= c.count;
      c.y /= c.count;
    }

    // Apply gentle attraction toward centroid
    for (const node of nodes) {
      const c = centroids.get(node.clusterId);
      if (!c || node.x == null || node.y == null) continue;
      const dx = c.x - node.x;
      const dy = c.y - node.y;
      node.x! += dx * alpha * strength;
      node.y! += dy * alpha * strength;
    }
  }

  force.initialize = () => {};

  return force;
}

export function useForceGraph({
  containerRef,
  elements,
  settings,
  onNodeClick,
  onNodeDoubleClick,
  onNodeHover,
  onNodeNudge,
  onPositionsChanged,
  highlightedClusterId,
  revealedNodes,
}: UseForceGraphParams) {
  const graphRef = useRef<ForceGraphMethods | null>(null);
  const hoveredNodeIdRef = useRef<string | null>(null);
  const neighborSetRef = useRef<Set<string>>(new Set());
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const highlightedRef = useRef(highlightedClusterId);
  highlightedRef.current = highlightedClusterId;
  const revealedRef = useRef(revealedNodes);
  revealedRef.current = revealedNodes;

  // Precompute adjacency for hover neighborhood lookup
  const adjacencyRef = useRef(new Map<string, Set<string>>());
  useEffect(() => {
    const adj = new Map<string, Set<string>>();
    for (const link of elements.links) {
      const sId = typeof link.source === "string" ? link.source : (link.source as unknown as ForceNode).id;
      const tId = typeof link.target === "string" ? link.target : (link.target as unknown as ForceNode).id;
      if (!adj.has(sId)) adj.set(sId, new Set());
      if (!adj.has(tId)) adj.set(tId, new Set());
      adj.get(sId)?.add(tId);
      adj.get(tId)?.add(sId);
    }
    adjacencyRef.current = adj;
  }, [elements.links]);

  const handleNodeHover = useCallback(
    (node: ForceNode | null) => {
      if (node) {
        hoveredNodeIdRef.current = node.id;
        neighborSetRef.current = adjacencyRef.current.get(node.id) ?? new Set();
        onNodeHover?.(node.id, node.x ?? 0, node.y ?? 0);
      } else {
        hoveredNodeIdRef.current = null;
        neighborSetRef.current = new Set();
        onNodeHover?.(null, 0, 0);
      }
    },
    [onNodeHover],
  );

  const handleNodeClick = useCallback(
    (node: ForceNode) => onNodeClick?.(node.id),
    [onNodeClick],
  );

  const handleNodeDragEnd = useCallback(
    (node: ForceNode) => {
      if (onNodeNudge && node.x != null && node.y != null) {
        onNodeNudge({
          nodeId: node.id,
          position: { x: node.x, y: node.y },
          clusterId: node.clusterId,
          timestamp: Date.now(),
        });
      }
      if (onPositionsChanged) {
        onPositionsChanged(snapshotPositions(elements.nodes));
      }
    },
    [onNodeNudge, onPositionsChanged, elements.nodes],
  );

  const handleEngineStop = useCallback(() => {
    if (onPositionsChanged) {
      onPositionsChanged(snapshotPositions(elements.nodes));
    }
  }, [onPositionsChanged, elements.nodes]);

  // Configure forces when settings change
  useEffect(() => {
    const fg = graphRef.current;
    if (!fg) return;

    fg.d3Force("charge", forceManyBody().strength(-settings.repulsion / 10));
    fg.d3Force("center", forceCenter().strength(settings.centerForce));
    fg.d3Force(
      "collide",
      forceCollide<ForceNode>().radius((n) => (n.size / 2) * settings.nodeScale + 2),
    );
    fg.d3Force("cluster", forceCluster(elements.nodes, 0.3) as any);
    fg.d3Force("link")?.distance?.(settings.linkDistance);
    fg.d3ReheatSimulation();
  }, [settings.linkDistance, settings.repulsion, settings.centerForce, settings.nodeScale, elements.nodes]);

  // Paint context for custom rendering
  const getPaintCtx = useCallback(
    (): PaintContext => ({
      nodeScale: settingsRef.current.nodeScale,
      labelThreshold: settingsRef.current.labelThreshold,
      hoveredNodeId: hoveredNodeIdRef.current,
      neighborSet: neighborSetRef.current,
      highlightedClusterId: highlightedRef.current,
    }),
    [],
  );

  const nodeCanvasObject = useCallback(
    (node: ForceNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
      // Hide unrevealed nodes
      if (revealedRef.current.size > 0 && !revealedRef.current.has(node.id)) return;
      paintNode(node, ctx, globalScale, getPaintCtx());
    },
    [getPaintCtx],
  );

  const linkCanvasObject = useCallback(
    (link: ForceLink, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const source = link.source as unknown as ForceNode;
      const target = link.target as unknown as ForceNode;
      // Hide links to unrevealed nodes
      if (revealedRef.current.size > 0) {
        if (!revealedRef.current.has(source.id) || !revealedRef.current.has(target.id)) return;
      }
      paintLink(link, ctx, globalScale, getPaintCtx());
    },
    [getPaintCtx],
  );

  // Public API
  const runLayout = useCallback(() => {
    graphRef.current?.d3ReheatSimulation();
  }, []);

  const zoomIn = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;
    const { k, x, y } = fg.zoom();
    fg.zoom(k * 1.3, 300);
  }, []);

  const zoomOut = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;
    const { k } = fg.zoom();
    fg.zoom(k / 1.3, 300);
  }, []);

  const fitToScreen = useCallback(() => {
    graphRef.current?.zoomToFit(300, 40);
  }, []);

  const getViewportBounds = useCallback((): ViewportBounds => {
    const fg = graphRef.current;
    if (!fg || !containerRef.current) {
      return { x: 0, y: 0, width: 0, height: 0 };
    }
    const { width, height } = containerRef.current.getBoundingClientRect();
    const topLeft = fg.screen2GraphCoords(0, 0);
    const bottomRight = fg.screen2GraphCoords(width, height);
    return {
      x: topLeft.x,
      y: topLeft.y,
      width: bottomRight.x - topLeft.x,
      height: bottomRight.y - topLeft.y,
    };
  }, [containerRef]);

  const getCurrentPositions = useCallback((): PositionMap => {
    return snapshotPositions(elements.nodes);
  }, [elements.nodes]);

  const centerAt = useCallback((x: number, y: number, ms = 300) => {
    graphRef.current?.centerAt(x, y, ms);
  }, []);

  return {
    ForceGraph2D,
    graphRef,
    nodeCanvasObject,
    linkCanvasObject,
    handleNodeClick,
    handleNodeHover,
    handleNodeDragEnd,
    handleEngineStop,
    runLayout,
    zoomIn,
    zoomOut,
    fitToScreen,
    getViewportBounds,
    getCurrentPositions,
    centerAt,
  };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useForceGraph.ts
git commit -m "feat(graph): add useForceGraph hook with 2D canvas renderer and custom painting"
```

---

### Task 7: Create GraphMinimap component (viewport thumbnail)

**Files:**
- Create: `desktop-ui/src/features/notes/components/GraphMinimap.tsx` (full rewrite)

- [ ] **Step 1: Create the new minimap component**

Replace the entire content of `desktop-ui/src/features/notes/components/GraphMinimap.tsx`:

```typescript
import { Map } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ForceNode, ForceLink } from "../hooks/useGraphElements";
import type { ViewportBounds } from "../hooks/useForceGraph";

interface GraphMinimapProps {
  nodes: ForceNode[];
  links: ForceLink[];
  viewportBounds: ViewportBounds;
  revealProgress: number;
  revealedNodes: Set<string>;
  visible: boolean;
  onToggle: () => void;
  onNavigate: (x: number, y: number) => void;
}

const MINIMAP_WIDTH = 180;
const MINIMAP_HEIGHT = 120;
const PADDING = 10;

export function GraphMinimap({
  nodes,
  links,
  viewportBounds,
  revealProgress,
  revealedNodes,
  visible,
  onToggle,
  onNavigate,
}: GraphMinimapProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const lastPaintRef = useRef(0);

  // Compute graph bounding box
  const getBounds = useCallback(() => {
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const n of nodes) {
      if (n.x == null || n.y == null) continue;
      if (n.x < minX) minX = n.x;
      if (n.x > maxX) maxX = n.x;
      if (n.y < minY) minY = n.y;
      if (n.y > maxY) maxY = n.y;
    }
    if (!Number.isFinite(minX)) return { minX: 0, minY: 0, maxX: 100, maxY: 100 };
    // Add padding
    const padX = (maxX - minX) * 0.1 || 50;
    const padY = (maxY - minY) * 0.1 || 50;
    return { minX: minX - padX, minY: minY - padY, maxX: maxX + padX, maxY: maxY + padY };
  }, [nodes]);

  const paint = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Throttle to ~10fps
    const now = Date.now();
    if (now - lastPaintRef.current < 100) return;
    lastPaintRef.current = now;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = MINIMAP_WIDTH * dpr;
    canvas.height = MINIMAP_HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    // Clear
    ctx.clearRect(0, 0, MINIMAP_WIDTH, MINIMAP_HEIGHT);

    const bounds = getBounds();
    const graphWidth = bounds.maxX - bounds.minX || 1;
    const graphHeight = bounds.maxY - bounds.minY || 1;

    const innerW = MINIMAP_WIDTH - PADDING * 2;
    const innerH = MINIMAP_HEIGHT - PADDING * 2;
    const scale = Math.min(innerW / graphWidth, innerH / graphHeight);

    const toMiniX = (x: number) => PADDING + (x - bounds.minX) * scale;
    const toMiniY = (y: number) => PADDING + (y - bounds.minY) * scale;

    // Draw links
    ctx.strokeStyle = "rgba(255,255,255,0.08)";
    ctx.lineWidth = 0.5;
    for (const link of links) {
      const source = link.source as unknown as ForceNode;
      const target = link.target as unknown as ForceNode;
      if (source.x == null || source.y == null || target.x == null || target.y == null) continue;
      ctx.beginPath();
      ctx.moveTo(toMiniX(source.x), toMiniY(source.y));
      ctx.lineTo(toMiniX(target.x), toMiniY(target.y));
      ctx.stroke();
    }

    // Draw nodes
    const isRevealing = revealedNodes.size > 0 && revealProgress < 1;
    for (const node of nodes) {
      if (node.x == null || node.y == null) continue;
      const r = Math.max(1.5, (node.size / 46) * 3);
      let alpha = 0.8;
      if (isRevealing && !revealedNodes.has(node.id)) alpha = 0.15;

      ctx.fillStyle =
        node.color +
        Math.round(alpha * 255)
          .toString(16)
          .padStart(2, "0");
      ctx.beginPath();
      ctx.arc(toMiniX(node.x), toMiniY(node.y), r, 0, Math.PI * 2);
      ctx.fill();
    }

    // Draw viewport rectangle
    if (viewportBounds.width > 0 && viewportBounds.height > 0) {
      const vx = toMiniX(viewportBounds.x);
      const vy = toMiniY(viewportBounds.y);
      const vw = viewportBounds.width * scale;
      const vh = viewportBounds.height * scale;

      ctx.strokeStyle = "rgba(167,139,250,0.6)";
      ctx.lineWidth = 1.5;
      ctx.strokeRect(vx, vy, vw, vh);
      ctx.fillStyle = "rgba(167,139,250,0.05)";
      ctx.fillRect(vx, vy, vw, vh);
    }
  }, [nodes, links, viewportBounds, revealProgress, revealedNodes, getBounds]);

  // Repaint on data change
  useEffect(() => {
    if (visible) paint();
  }, [visible, paint]);

  // Also repaint on requestAnimationFrame for smooth viewport tracking
  useEffect(() => {
    if (!visible) return;
    let animId: number;
    const loop = () => {
      paint();
      animId = requestAnimationFrame(loop);
    };
    animId = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(animId);
  }, [visible, paint]);

  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const clickX = e.clientX - rect.left;
      const clickY = e.clientY - rect.top;

      const bounds = getBounds();
      const graphWidth = bounds.maxX - bounds.minX || 1;
      const graphHeight = bounds.maxY - bounds.minY || 1;
      const innerW = MINIMAP_WIDTH - PADDING * 2;
      const innerH = MINIMAP_HEIGHT - PADDING * 2;
      const scale = Math.min(innerW / graphWidth, innerH / graphHeight);

      const graphX = (clickX - PADDING) / scale + bounds.minX;
      const graphY = (clickY - PADDING) / scale + bounds.minY;

      onNavigate(graphX, graphY);
    },
    [getBounds, onNavigate],
  );

  return (
    <div className="absolute bottom-4 left-4 z-10">
      <button
        type="button"
        onClick={onToggle}
        className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground mb-1"
        aria-label={visible ? "Hide minimap" : "Show minimap"}
      >
        <Map size={14} />
      </button>
      {visible && (
        <div className="glass-card p-1 cursor-crosshair">
          <canvas
            ref={canvasRef}
            width={MINIMAP_WIDTH}
            height={MINIMAP_HEIGHT}
            style={{ width: MINIMAP_WIDTH, height: MINIMAP_HEIGHT, display: "block" }}
            onClick={handleClick}
          />
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphMinimap.tsx
git commit -m "feat(graph): rewrite GraphMinimap as viewport-rectangle thumbnail"
```

---

### Task 8: Update GraphLegend (remove Cytoscape dependency)

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphLegend.tsx`

- [ ] **Step 1: Update the import path**

Change the import in `GraphLegend.tsx` from:
```typescript
import type { ClusterInfo } from "../hooks/useCytoscapeElements";
```
to:
```typescript
import type { ClusterInfo } from "../hooks/useGraphElements";
```

This is the only change needed — the component's props and logic are already callback-based. The `onHighlight` callback is handled by the parent (GraphView) which will set `highlightedClusterId` state consumed by paint functions.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphLegend.tsx
git commit -m "refactor(graph): update GraphLegend import to useGraphElements"
```

---

### Task 9: Update GraphToolbar (add ClusteringMode switcher)

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphToolbar.tsx`

- [ ] **Step 1: Add ClusteringMode switcher and Brain View toggle**

Replace the full content of `desktop-ui/src/features/notes/components/GraphToolbar.tsx`:

```typescript
import { Brain, Search } from "lucide-react";
import type { SmartView } from "../hooks/useGraphData";

interface GraphToolbarProps {
  view: SmartView;
  onViewChange: (view: SmartView) => void;
  hopRadius: number;
  onHopRadiusChange: (r: number) => void;
  searchQuery: string;
  onSearchChange: (q: string) => void;
  clusteringMode: "notebook" | "semantic";
  onClusteringModeChange: (mode: "notebook" | "semantic") => void;
  renderMode: "2d" | "3d";
  onRenderModeChange: (mode: "2d" | "3d") => void;
}

const VIEW_OPTIONS: { value: SmartView; label: string }[] = [
  { value: "local", label: "Local" },
  { value: "full", label: "Full" },
  { value: "by-tag", label: "By Tag" },
  { value: "by-notebook", label: "By Notebook" },
  { value: "orphans", label: "Orphans" },
];

const CLUSTERING_OPTIONS: { value: "notebook" | "semantic"; label: string }[] = [
  { value: "notebook", label: "Notebook" },
  { value: "semantic", label: "Semantic" },
];

export function GraphToolbar({
  view,
  onViewChange,
  hopRadius,
  onHopRadiusChange,
  searchQuery,
  onSearchChange,
  clusteringMode,
  onClusteringModeChange,
  renderMode,
  onRenderModeChange,
}: GraphToolbarProps) {
  return (
    <div className="flex items-center gap-2 px-3 py-2 shrink-0">
      {/* Smart view pills */}
      <div className="flex items-center gap-0.5 bg-card rounded-lg p-0.5">
        {VIEW_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onViewChange(opt.value)}
            className={`px-2.5 py-1 text-xs rounded-md transition-all ${
              view === opt.value
                ? "bg-brand/20 text-brand font-medium shadow-sm"
                : "text-muted-foreground hover:text-foreground hover:bg-accent"
            }`}
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* Hop radius selector (only for local view) */}
      {view === "local" && (
        <div className="flex items-center gap-1 text-xs text-muted-foreground">
          <span>Hops:</span>
          <div className="flex items-center gap-0.5 bg-card rounded-lg p-0.5">
            {[1, 2, 3].map((r) => (
              <button
                key={r}
                type="button"
                onClick={() => onHopRadiusChange(r)}
                className={`size-6 rounded-md text-xs flex items-center justify-center transition-all ${
                  hopRadius === r
                    ? "bg-brand/20 text-brand font-medium"
                    : "text-muted-foreground hover:text-foreground hover:bg-accent"
                }`}
              >
                {r}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Clustering mode switcher */}
      <div className="flex items-center gap-0.5 bg-card rounded-lg p-0.5">
        {CLUSTERING_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onClusteringModeChange(opt.value)}
            className={`px-2 py-1 text-xs rounded-md transition-all ${
              clusteringMode === opt.value
                ? "bg-brand/20 text-brand font-medium shadow-sm"
                : "text-muted-foreground hover:text-foreground hover:bg-accent"
            } ${opt.value === "semantic" ? "opacity-50 cursor-not-allowed" : ""}`}
            disabled={opt.value === "semantic"}
            title={opt.value === "semantic" ? "Coming soon — requires semantic community detection" : undefined}
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Brain View toggle */}
      <button
        type="button"
        onClick={() => onRenderModeChange(renderMode === "2d" ? "3d" : "2d")}
        className={`flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-lg transition-all ${
          renderMode === "3d"
            ? "bg-brand/20 text-brand font-medium shadow-sm"
            : "text-muted-foreground hover:text-foreground hover:bg-accent"
        }`}
        title={renderMode === "3d" ? "Exit Brain View" : "Enter Brain View"}
      >
        <Brain size={14} />
        {renderMode === "3d" ? "Exit Brain View" : "Brain View"}
      </button>

      {/* Search input */}
      <div className="relative">
        <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-dim pointer-events-none" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Filter nodes..."
          className="w-40 pl-7 pr-2 py-1 text-xs rounded-lg bg-card border border-border-subtle text-foreground placeholder:text-dim outline-none focus:border-brand/40 transition-colors"
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphToolbar.tsx
git commit -m "feat(graph): add ClusteringMode switcher and Brain View toggle to toolbar"
```

---

### Task 10: Update GraphSettingsPopover (add new controls)

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx`

- [ ] **Step 1: Add reveal speed and 3D settings**

Replace the full content of `desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx`:

```typescript
import { RotateCcw } from "lucide-react";
import type { GraphSettings } from "../hooks/useGraphSettings";

interface GraphSettingsPopoverProps {
  settings: GraphSettings;
  defaults: GraphSettings;
  onChange: (partial: Partial<GraphSettings>) => void;
  onReset: () => void;
}

function Slider({
  label,
  value,
  min,
  max,
  step,
  unit,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  unit?: string;
  onChange: (v: number) => void;
}) {
  const pct = ((value - min) / (max - min)) * 100;

  return (
    <div className="flex items-center gap-3 h-7">
      <span className="text-[11px] text-muted-foreground w-[90px] shrink-0">{label}</span>
      <div className="flex-1 relative flex items-center h-5">
        <div className="absolute inset-x-0 top-1/2 -translate-y-1/2 h-[4px] rounded-full bg-muted overflow-hidden">
          <div className="h-full rounded-full bg-brand/50" style={{ width: `${pct}%` }} />
        </div>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          style={{ WebkitAppearance: "none", appearance: "none", background: "transparent" }}
          className="relative z-10 w-full h-5 cursor-pointer outline-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-brand [&::-webkit-slider-thumb]:cursor-pointer [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-background"
        />
      </div>
      <span className="text-2xs text-muted-foreground tabular-nums w-[36px] text-right shrink-0">
        {value}
        {unit}
      </span>
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-3 h-7">
      <span className="text-[11px] text-muted-foreground flex-1">{label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative w-[34px] h-[18px] rounded-full transition-colors shrink-0 ${
          checked ? "bg-brand" : "bg-muted"
        }`}
      >
        <span
          className={`absolute top-[3px] size-3 rounded-full bg-background transition-all ${
            checked ? "left-[19px]" : "left-[3px]"
          }`}
        />
      </button>
    </div>
  );
}

const REVEAL_OPTIONS: { value: GraphSettings["revealSpeed"]; label: string }[] = [
  { value: "instant", label: "Instant" },
  { value: "balanced", label: "Balanced" },
  { value: "cinematic", label: "Cinematic" },
];

export function GraphSettingsPopover({
  settings,
  defaults,
  onChange,
  onReset,
}: GraphSettingsPopoverProps) {
  const isDefault =
    settings.linkDistance === defaults.linkDistance &&
    settings.repulsion === defaults.repulsion &&
    settings.centerForce === defaults.centerForce &&
    settings.nodeScale === defaults.nodeScale;

  return (
    <div className="w-[280px]">
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-2xs font-semibold text-muted-foreground uppercase tracking-wider">
          Settings
        </span>
        {!isDefault && (
          <button
            type="button"
            onClick={onReset}
            className="flex items-center gap-1 text-2xs text-muted-foreground hover:text-foreground transition-colors"
          >
            <RotateCcw size={9} />
            Reset
          </button>
        )}
      </div>

      {/* Sliders */}
      <div className="space-y-0.5">
        <Slider
          label="Link Distance"
          value={settings.linkDistance}
          min={40}
          max={300}
          step={10}
          unit="px"
          onChange={(v) => onChange({ linkDistance: v })}
        />
        <Slider
          label="Repulsion"
          value={settings.repulsion}
          min={1000}
          max={30000}
          step={500}
          onChange={(v) => onChange({ repulsion: v })}
        />
        <Slider
          label="Center Force"
          value={settings.centerForce}
          min={0}
          max={1}
          step={0.05}
          onChange={(v) => onChange({ centerForce: v })}
        />
        <Slider
          label="Node Size"
          value={settings.nodeScale}
          min={0.5}
          max={2}
          step={0.1}
          unit="×"
          onChange={(v) => onChange({ nodeScale: v })}
        />
        <Slider
          label="Label Threshold"
          value={settings.labelThreshold}
          min={0.1}
          max={1.5}
          step={0.1}
          unit="×"
          onChange={(v) => onChange({ labelThreshold: v })}
        />
      </div>

      {/* Toggles */}
      <div className="mt-2 pt-2 border-t border-border-subtle space-y-0.5">
        <Toggle
          label="Show Arrows"
          checked={settings.showArrows}
          onChange={(v) => onChange({ showArrows: v })}
        />
        <Toggle
          label="Show Orphan Nodes"
          checked={settings.showOrphans}
          onChange={(v) => onChange({ showOrphans: v })}
        />
        <Toggle
          label="Show Minimap"
          checked={settings.showMinimap}
          onChange={(v) => onChange({ showMinimap: v })}
        />
        {settings.renderMode === "3d" && (
          <Toggle
            label="Idle Rotation"
            checked={settings.idleRotation}
            onChange={(v) => onChange({ idleRotation: v })}
          />
        )}
      </div>

      {/* Reveal Speed */}
      <div className="mt-2 pt-2 border-t border-border-subtle">
        <div className="flex items-center gap-3 h-7">
          <span className="text-[11px] text-muted-foreground w-[90px] shrink-0">Reveal Speed</span>
          <div className="flex items-center gap-0.5 bg-muted rounded-lg p-0.5">
            {REVEAL_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                type="button"
                onClick={() => onChange({ revealSpeed: opt.value })}
                className={`px-2 py-0.5 text-2xs rounded-md transition-all ${
                  settings.revealSpeed === opt.value
                    ? "bg-brand/20 text-brand font-medium"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx
git commit -m "feat(graph): add reveal speed, minimap, and 3D settings to popover"
```

---

### Task 11: Rewire GraphView orchestrator

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`

This is the largest task — rewires the main component to use all new hooks and renders `ForceGraph2D` instead of Cytoscape.

- [ ] **Step 1: Rewrite GraphView.tsx**

Replace the full content of `desktop-ui/src/features/notes/components/GraphView.tsx`:

```typescript
import { useClickOutside } from "@shared/hooks/useClickOutside";
import type { Note, Notebook } from "@shared/types";
import { Maximize2, Minus, Plus, RotateCcw, Settings2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ForceGraph2D from "react-force-graph-2d";
import { useForceGraph, type ViewportBounds } from "../hooks/useForceGraph";
import { useGraphElements } from "../hooks/useGraphElements";
import type { SmartView } from "../hooks/useGraphData";
import { useGraphData } from "../hooks/useGraphData";
import { useGraphPositionCache } from "../hooks/useGraphPositionCache";
import { useGraphSettings } from "../hooks/useGraphSettings";
import { useWaveReveal } from "../hooks/useWaveReveal";
import { computeBfsWaves, selectHub } from "../lib/graphBfs";
import { GraphLegend } from "./GraphLegend";
import { GraphMinimap } from "./GraphMinimap";
import { GraphNodeTooltip } from "./GraphNodeTooltip";
import { GraphSettingsPopover } from "./GraphSettingsPopover";
import { GraphToolbar } from "./GraphToolbar";

interface GraphViewProps {
  notes: Note[];
  notebooks: Notebook[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  onOpenInEditor?: (id: string) => void;
}

export function GraphView({
  notes,
  notebooks,
  activeNoteId,
  onSelectNote,
  onOpenInEditor,
}: GraphViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  const [smartView, setSmartView] = useState<SmartView>("full");
  const [hopRadius, setHopRadius] = useState(2);
  const [searchQuery, setSearchQuery] = useState("");
  const [tooltip, setTooltip] = useState<{ nodeId: string; x: number; y: number } | null>(null);
  const [hiddenClusters, setHiddenClusters] = useState<Set<string>>(new Set());
  const [highlightedClusterId, setHighlightedClusterId] = useState<string | null>(null);
  const [viewportBounds, setViewportBounds] = useState<ViewportBounds>({ x: 0, y: 0, width: 0, height: 0 });

  // Settings popover
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);
  useClickOutside(settingsRef, () => setSettingsOpen(false), settingsOpen);

  const { settings, setSettings, resetSettings, defaults } = useGraphSettings();

  // Data pipeline
  const { nodes: rawNodes, links: rawLinks } = useGraphData(
    smartView,
    notes,
    activeNoteId,
    hopRadius,
  );

  // Search filter
  let filteredNodes = searchQuery
    ? rawNodes.filter((n) => n.title.toLowerCase().includes(searchQuery.toLowerCase()))
    : rawNodes;

  // Orphan filter
  if (!settings.showOrphans) {
    filteredNodes = filteredNodes.filter((n) => n.linkCount > 0);
  }

  const filteredNodeIds = new Set(filteredNodes.map((n) => n.id));
  const filteredLinks = rawLinks.filter((l) => {
    const sId = typeof l.source === "string" ? l.source : l.source.id;
    const tId = typeof l.target === "string" ? l.target : l.target.id;
    return filteredNodeIds.has(sId) && filteredNodeIds.has(tId);
  });

  const elements = useGraphElements({
    nodes: filteredNodes,
    links: filteredLinks,
    notebooks,
    clusteringMode: settings.clusteringMode,
    activeNoteId,
  });

  // Apply cluster filtering
  const visibleElements = useMemo(() => {
    if (hiddenClusters.size === 0) return elements;
    const visibleNodes = elements.nodes.filter((n) => !hiddenClusters.has(n.clusterId));
    const visibleNodeIds = new Set(visibleNodes.map((n) => n.id));
    const visibleLinks = elements.links.filter((l) => {
      const sId = typeof l.source === "string" ? l.source : (l.source as any).id;
      const tId = typeof l.target === "string" ? l.target : (l.target as any).id;
      return visibleNodeIds.has(sId) && visibleNodeIds.has(tId);
    });
    return {
      ...elements,
      nodes: visibleNodes,
      links: visibleLinks,
      clusters: elements.clusters.filter((c) => !hiddenClusters.has(c.id)),
    };
  }, [elements, hiddenClusters]);

  // Position cache
  const { loadPositions, savePositions } = useGraphPositionCache(smartView, elements.fingerprint);
  const [cachedPositions, setCachedPositions] = useState<Record<string, { x: number; y: number }> | null>(null);
  const [cacheReady, setCacheReady] = useState(false);
  const initialRevealDone = useRef(false);

  useEffect(() => {
    setCacheReady(false);
    setCachedPositions(null);
    initialRevealDone.current = false;
    loadPositions().then((pos) => {
      setCachedPositions(pos);
      setCacheReady(true);
    });
  }, [loadPositions]);

  // Wave reveal
  const waveReveal = useWaveReveal(settings.revealSpeed);

  // Trigger initial reveal once cache is ready
  useEffect(() => {
    if (!cacheReady || initialRevealDone.current || visibleElements.nodes.length === 0) return;
    initialRevealDone.current = true;

    const hubId = selectHub(
      visibleElements.nodes.map((n) => ({ id: n.id, linkCount: n.linkCount, title: n.label })),
      activeNoteId,
    );

    // Apply cached positions
    if (cachedPositions) {
      for (const node of visibleElements.nodes) {
        const pos = cachedPositions[node.id];
        if (pos) {
          node.x = pos.x;
          node.y = pos.y;
          node.fx = pos.x;
          node.fy = pos.y;
        }
      }
      // Release pins after a short delay so the reveal animation can play
      setTimeout(() => {
        for (const node of visibleElements.nodes) {
          node.fx = null;
          node.fy = null;
        }
      }, 500);
    }

    waveReveal.revealWave(hubId, visibleElements, cachedPositions);
  }, [cacheReady, visibleElements, cachedPositions, activeNoteId, waveReveal]);

  // Force graph hook
  const {
    graphRef,
    nodeCanvasObject,
    linkCanvasObject,
    handleNodeClick,
    handleNodeHover,
    handleNodeDragEnd,
    handleEngineStop,
    runLayout,
    zoomIn,
    zoomOut,
    fitToScreen,
    getViewportBounds,
    centerAt,
  } = useForceGraph({
    containerRef,
    elements: visibleElements,
    settings,
    onNodeClick: onSelectNote,
    onNodeHover: useCallback((id: string | null, x: number, y: number) => {
      if (id) {
        setTooltip({ nodeId: id, x, y });
      } else {
        setTooltip(null);
      }
    }, []),
    onPositionsChanged: savePositions,
    highlightedClusterId,
    revealedNodes: waveReveal.revealedNodes,
  });

  // Update viewport bounds for minimap
  useEffect(() => {
    const interval = setInterval(() => {
      setViewportBounds(getViewportBounds());
    }, 200);
    return () => clearInterval(interval);
  }, [getViewportBounds]);

  // Legend handlers
  const handleLegendHighlight = useCallback((clusterId: string | null) => {
    setHighlightedClusterId(clusterId);
  }, []);

  const handleToggleCluster = useCallback((clusterId: string) => {
    setHiddenClusters((prev) => {
      const next = new Set(prev);
      if (next.has(clusterId)) next.delete(clusterId);
      else next.add(clusterId);
      return next;
    });
  }, []);

  const handleShowAll = useCallback(() => {
    setHiddenClusters(new Set());
  }, []);

  // Minimap navigate
  const handleMinimapNavigate = useCallback(
    (x: number, y: number) => centerAt(x, y),
    [centerAt],
  );

  const nodeMap = new Map(visibleElements.nodes.map((n) => [n.id, n]));

  // Empty state
  if (notes.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-muted-foreground">
        <p className="text-sm font-medium">Your knowledge graph will appear here</p>
        <p className="text-xs text-dim">Create your first note to get started</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <GraphToolbar
        view={smartView}
        onViewChange={setSmartView}
        hopRadius={hopRadius}
        onHopRadiusChange={setHopRadius}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        clusteringMode={settings.clusteringMode}
        onClusteringModeChange={(mode) => setSettings({ clusteringMode: mode })}
        renderMode={settings.renderMode}
        onRenderModeChange={(mode) => setSettings({ renderMode: mode })}
      />

      <div
        className="flex-1 relative min-h-0 bg-background"
        style={{
          backgroundImage: "radial-gradient(circle, var(--border) 0.5px, transparent 0.5px)",
          backgroundSize: "20px 20px",
        }}
      >
        <div ref={containerRef} style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}>
          {settings.renderMode === "2d" && cacheReady && (
            <ForceGraph2D
              ref={graphRef}
              graphData={{ nodes: visibleElements.nodes, links: visibleElements.links }}
              nodeId="id"
              nodeCanvasObject={nodeCanvasObject}
              nodeCanvasObjectMode={() => "replace"}
              linkCanvasObject={linkCanvasObject}
              linkCanvasObjectMode={() => "replace"}
              linkDirectionalParticles={(link: any) => Math.ceil(link.weight || 1)}
              linkDirectionalParticleSpeed={0.005}
              linkDirectionalParticleColor={(link: any) => link.color || "#6B7280"}
              linkDirectionalParticleWidth={1.5}
              linkDirectionalArrowLength={settings.showArrows ? 5 : 0}
              linkDirectionalArrowRelPos={1}
              onNodeClick={handleNodeClick}
              onNodeHover={handleNodeHover}
              onNodeDragEnd={handleNodeDragEnd}
              onEngineStop={handleEngineStop}
              enableNodeDrag
              enableZoomInteraction
              enablePanInteraction
              minZoom={0.1}
              maxZoom={5}
              width={containerRef.current?.clientWidth}
              height={containerRef.current?.clientHeight}
              backgroundColor="transparent"
              cooldownTicks={cachedPositions ? 0 : 200}
              warmupTicks={cachedPositions ? 0 : 50}
            />
          )}
          {settings.renderMode === "3d" && (
            <div className="absolute inset-0 flex items-center justify-center text-muted-foreground text-sm">
              Brain View coming soon
            </div>
          )}
        </div>

        {/* Loading overlay */}
        {!cacheReady && (
          <div className="absolute inset-0 flex items-center justify-center text-muted-foreground text-sm z-20">
            Loading graph...
          </div>
        )}

        {/* Legend */}
        <GraphLegend
          clusters={elements.clusters}
          hiddenClusters={hiddenClusters}
          onToggleCluster={handleToggleCluster}
          onShowAll={handleShowAll}
          onHighlight={handleLegendHighlight}
        />

        {/* Minimap */}
        <GraphMinimap
          nodes={visibleElements.nodes}
          links={visibleElements.links}
          viewportBounds={viewportBounds}
          revealProgress={waveReveal.revealProgress}
          revealedNodes={waveReveal.revealedNodes}
          visible={settings.showMinimap}
          onToggle={() => setSettings({ showMinimap: !settings.showMinimap })}
          onNavigate={handleMinimapNavigate}
        />

        {/* Controls (bottom-right) */}
        <div className="absolute bottom-4 right-4 z-10 flex flex-col gap-1">
          <div className="relative" ref={settingsRef}>
            <button
              type="button"
              onClick={() => setSettingsOpen(!settingsOpen)}
              className={`size-7 glass-button flex items-center justify-center transition-colors ${
                settingsOpen ? "text-brand" : "text-muted-foreground hover:text-foreground"
              }`}
              aria-label="Graph settings"
            >
              <Settings2 size={14} />
            </button>
            {settingsOpen && (
              <div className="absolute bottom-9 right-0 glass-card p-3">
                <GraphSettingsPopover
                  settings={settings}
                  defaults={defaults}
                  onChange={setSettings}
                  onReset={resetSettings}
                />
              </div>
            )}
          </div>

          <button type="button" onClick={zoomIn} className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground" aria-label="Zoom in">
            <Plus size={14} />
          </button>
          <button type="button" onClick={zoomOut} className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground" aria-label="Zoom out">
            <Minus size={14} />
          </button>
          <button type="button" onClick={fitToScreen} className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground" aria-label="Fit to screen">
            <Maximize2 size={14} />
          </button>
          <button type="button" onClick={runLayout} className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground" aria-label="Re-layout">
            <RotateCcw size={14} />
          </button>
        </div>

        {/* Tooltip */}
        {(() => {
          if (!tooltip) return null;
          const tooltipNode = nodeMap.get(tooltip.nodeId);
          if (!tooltipNode) return null;
          return <GraphNodeTooltip node={tooltipNode} x={tooltip.x} y={tooltip.y} />;
        })()}
      </div>
    </div>
  );
}
```

Note: The `GraphNodeTooltip` expects a node with `title`, `bodyPreview`, `linkCount`, `tags` fields. `ForceNode` has `label` instead of `title`. Either update the tooltip component to use `label`, or pass a mapped object. Check `GraphNodeTooltip.tsx` and adjust if needed — the field names should match.

- [ ] **Step 2: Verify the tooltip node shape**

Read `desktop-ui/src/features/notes/components/GraphNodeTooltip.tsx` and check which fields it uses. If it uses `title`, you need to either:
- Add a `title` getter on `ForceNode` (just alias `label`), or
- Update the tooltip to use `node.label` instead of `node.title`

Make whichever change is simpler.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(graph): rewire GraphView to use react-force-graph-2d with custom painters"
```

---

### Task 12: Create graphMaterials.ts (Three.js materials for 3D)

**Files:**
- Create: `desktop-ui/src/features/notes/lib/graphMaterials.ts`

- [ ] **Step 1: Create the materials module**

Create `desktop-ui/src/features/notes/lib/graphMaterials.ts`:

```typescript
import {
  Color,
  LineBasicMaterial,
  MeshStandardMaterial,
  SphereGeometry,
} from "three";

/**
 * Create a Three.js material for a graph node.
 * Uses emissive color to drive bloom post-processing.
 */
export function createNodeMaterial(
  hexColor: string,
  emissiveIntensity: number,
): MeshStandardMaterial {
  const color = new Color(hexColor);
  return new MeshStandardMaterial({
    color,
    emissive: color,
    emissiveIntensity,
    transparent: true,
    opacity: 0.9,
    roughness: 0.4,
    metalness: 0.1,
  });
}

/**
 * Create a Three.js sphere geometry for a graph node.
 * Segment count scales with size for visual quality.
 */
export function createNodeGeometry(size: number): SphereGeometry {
  const radius = size / 2;
  const segments = radius > 15 ? 24 : radius > 8 ? 16 : 12;
  return new SphereGeometry(radius, segments, segments);
}

/**
 * Create a Three.js material for a graph link.
 */
export function createLinkMaterial(
  hexColor: string,
  opacity: number,
): LineBasicMaterial {
  return new LineBasicMaterial({
    color: new Color(hexColor),
    transparent: true,
    opacity: Math.min(opacity, 1),
  });
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/lib/graphMaterials.ts
git commit -m "feat(graph): add Three.js material factories for 3D Brain View"
```

---

### Task 13: Create useBrainView hook and GraphBrainView component (3D renderer)

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useBrainView.ts`
- Create: `desktop-ui/src/features/notes/components/GraphBrainView.tsx`

- [ ] **Step 1: Create useBrainView hook**

Create `desktop-ui/src/features/notes/hooks/useBrainView.ts`:

```typescript
import { useCallback, useEffect, useRef } from "react";
import type { ForceGraphMethods as ForceGraph3DMethods } from "react-force-graph-3d";
import { Mesh } from "three";
import { createNodeGeometry, createNodeMaterial } from "../lib/graphMaterials";
import type { ForceNode } from "./useGraphElements";
import type { GraphSettings } from "./useGraphSettings";

interface UseBrainViewParams {
  settings: GraphSettings;
}

export function useBrainView({ settings }: UseBrainViewParams) {
  const graphRef = useRef<ForceGraph3DMethods | null>(null);
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Setup post-processing after mount
  const setupPostProcessing = useCallback((fg: ForceGraph3DMethods) => {
    const renderer = fg.renderer();
    const scene = fg.scene();
    const camera = fg.camera();

    if (!renderer || !scene || !camera) return;

    // Set background color
    scene.background = null;
    renderer.setClearColor(0x07070d, 1);

    // Add ambient light for emissive materials
    const { AmbientLight, PointLight } = require("three");
    scene.add(new AmbientLight(0xffffff, 0.4));
    const pointLight = new PointLight(0xffffff, 0.8);
    pointLight.position.set(0, 200, 200);
    scene.add(pointLight);

    // Bloom post-processing (loaded dynamically to avoid SSR issues)
    try {
      const { EffectComposer } = require("three/examples/jsm/postprocessing/EffectComposer.js");
      const { RenderPass } = require("three/examples/jsm/postprocessing/RenderPass.js");
      const { UnrealBloomPass } = require("three/examples/jsm/postprocessing/UnrealBloomPass.js");
      const { Vector2 } = require("three");

      const composer = new EffectComposer(renderer);
      composer.addPass(new RenderPass(scene, camera));

      const bloomPass = new UnrealBloomPass(
        new Vector2(window.innerWidth, window.innerHeight),
        1.5,  // strength
        0.4,  // radius
        0.2,  // threshold
      );
      composer.addPass(bloomPass);

      // Override the render loop to use composer
      fg.postProcessingComposer(composer);
    } catch {
      // Bloom not available — render without post-processing
    }
  }, []);

  // Custom node rendering
  const nodeThreeObject = useCallback(
    (node: ForceNode) => {
      const emissiveIntensity = 0.3 + (Math.min(node.linkCount, 15) / 15) * 0.7;
      const scaledSize = node.size * settings.nodeScale;
      const geometry = createNodeGeometry(scaledSize);
      const material = createNodeMaterial(node.color, emissiveIntensity);
      return new Mesh(geometry, material);
    },
    [settings.nodeScale],
  );

  // Auto-rotate when idle
  useEffect(() => {
    const fg = graphRef.current;
    if (!fg) return;

    const controls = fg.controls();
    if (!controls) return;

    if (settings.idleRotation) {
      controls.autoRotate = true;
      controls.autoRotateSpeed = 0.4; // ~0.2°/s
    } else {
      controls.autoRotate = false;
    }
  }, [settings.idleRotation]);

  return {
    graphRef,
    nodeThreeObject,
    setupPostProcessing,
  };
}
```

- [ ] **Step 2: Create GraphBrainView component**

Create `desktop-ui/src/features/notes/components/GraphBrainView.tsx`:

```typescript
import { useCallback, useEffect, useRef } from "react";
import ForceGraph3D from "react-force-graph-3d";
import type { ForceNode, ForceLink, GraphElements } from "../hooks/useGraphElements";
import type { GraphSettings } from "../hooks/useGraphSettings";
import { useBrainView } from "../hooks/useBrainView";

interface GraphBrainViewProps {
  elements: GraphElements;
  settings: GraphSettings;
  width: number;
  height: number;
  onNodeClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
}

export function GraphBrainView({
  elements,
  settings,
  width,
  height,
  onNodeClick,
  onNodeHover,
}: GraphBrainViewProps) {
  const { graphRef, nodeThreeObject, setupPostProcessing } = useBrainView({ settings });
  const initialized = useRef(false);

  const handleRef = useCallback(
    (fg: any) => {
      graphRef.current = fg;
      if (fg && !initialized.current) {
        initialized.current = true;
        setupPostProcessing(fg);
      }
    },
    [graphRef, setupPostProcessing],
  );

  const handleNodeClick = useCallback(
    (node: ForceNode) => onNodeClick?.(node.id),
    [onNodeClick],
  );

  const handleNodeHover = useCallback(
    (node: ForceNode | null) => {
      onNodeHover?.(node?.id ?? null, 0, 0);
    },
    [onNodeHover],
  );

  return (
    <ForceGraph3D
      ref={handleRef}
      graphData={{ nodes: elements.nodes, links: elements.links }}
      nodeId="id"
      nodeThreeObject={nodeThreeObject}
      nodeThreeObjectExtend={false}
      linkColor={(link: any) => link.color || "#6B7280"}
      linkOpacity={0.3}
      linkDirectionalParticles={(link: any) => Math.ceil(link.weight || 1)}
      linkDirectionalParticleSpeed={0.005}
      linkDirectionalParticleColor={(link: any) => link.color || "#6B7280"}
      onNodeClick={handleNodeClick}
      onNodeHover={handleNodeHover}
      enableNodeDrag
      enableNavigationControls
      width={width}
      height={height}
      backgroundColor="#07070d"
    />
  );
}
```

- [ ] **Step 3: Update GraphView to render GraphBrainView instead of placeholder**

In `desktop-ui/src/features/notes/components/GraphView.tsx`, replace:
```typescript
          {settings.renderMode === "3d" && (
            <div className="absolute inset-0 flex items-center justify-center text-muted-foreground text-sm">
              Brain View coming soon
            </div>
          )}
```

with:
```typescript
          {settings.renderMode === "3d" && cacheReady && (
            <GraphBrainView
              elements={visibleElements}
              settings={settings}
              width={containerRef.current?.clientWidth ?? 0}
              height={containerRef.current?.clientHeight ?? 0}
              onNodeClick={onSelectNote}
              onNodeHover={useCallback((id: string | null, x: number, y: number) => {
                if (id) setTooltip({ nodeId: id, x, y });
                else setTooltip(null);
              }, [])}
            />
          )}
```

And add the import at the top:
```typescript
import { GraphBrainView } from "./GraphBrainView";
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useBrainView.ts desktop-ui/src/features/notes/components/GraphBrainView.tsx desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(graph): add 3D Brain View with Three.js bloom post-processing"
```

---

### Task 14: Create useGraphTheme hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useGraphTheme.ts`

- [ ] **Step 1: Create the theme hook**

Create `desktop-ui/src/features/notes/hooks/useGraphTheme.ts`:

```typescript
import { useEffect, useState } from "react";

export interface GraphTheme {
  isDark: boolean;
  backgroundColor: string;
  edgeColor: string;
  edgeOpacity: number;
  labelColor: string;
  dimmedOpacity: number;
}

/**
 * Resolves the current theme for graph rendering.
 * Watches `data-theme` attribute on <html> for light/dark switches.
 */
export function useGraphTheme(): GraphTheme {
  const [isDark, setIsDark] = useState(() => {
    return document.documentElement.getAttribute("data-theme") !== "retro";
  });

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark(document.documentElement.getAttribute("data-theme") !== "retro");
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  return {
    isDark,
    backgroundColor: isDark ? "transparent" : "transparent",
    edgeColor: isDark ? "#4B5563" : "#9CA3AF",
    edgeOpacity: isDark ? 0.35 : 0.5,
    labelColor: isDark ? "rgba(255,255,255,0.7)" : "rgba(0,0,0,0.7)",
    dimmedOpacity: 0.12,
  };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphTheme.ts
git commit -m "feat(graph): add useGraphTheme hook for light/dark theme detection"
```

---

### Task 15: Delete old Cytoscape files

**Files:**
- Delete: `desktop-ui/src/features/notes/hooks/useCytoscapeGraph.ts`
- Delete: `desktop-ui/src/features/notes/hooks/useCytoscapeElements.ts`
- Delete: `desktop-ui/src/features/notes/hooks/useCytoscapeTheme.ts`
- Delete: `desktop-ui/src/features/notes/hooks/useColaPhysics.ts`
- Delete: `desktop-ui/src/features/notes/hooks/useProgressiveReveal.ts`
- Delete: `desktop-ui/src/features/notes/lib/elementDiff.ts`
- Delete: `desktop-ui/src/features/notes/lib/elementDiff.test.ts`
- Delete: `desktop-ui/src/features/notes/lib/graphUtils.ts`

- [ ] **Step 1: Delete all old Cytoscape files**

Run:
```bash
rm desktop-ui/src/features/notes/hooks/useCytoscapeGraph.ts \
   desktop-ui/src/features/notes/hooks/useCytoscapeElements.ts \
   desktop-ui/src/features/notes/hooks/useCytoscapeTheme.ts \
   desktop-ui/src/features/notes/hooks/useColaPhysics.ts \
   desktop-ui/src/features/notes/hooks/useProgressiveReveal.ts \
   desktop-ui/src/features/notes/lib/elementDiff.ts \
   desktop-ui/src/features/notes/lib/elementDiff.test.ts \
   desktop-ui/src/features/notes/lib/graphUtils.ts
```

- [ ] **Step 2: Verify no remaining imports of deleted files**

Run:
```bash
cd desktop-ui && grep -r "useCytoscapeGraph\|useCytoscapeElements\|useCytoscapeTheme\|useColaPhysics\|useProgressiveReveal\|elementDiff\|graphUtils" src/ --include="*.ts" --include="*.tsx" || echo "No stale imports found"
```

Expected: "No stale imports found"

If any stale imports are found, fix them (they should all be in `GraphView.tsx` which was already rewritten in Task 11).

- [ ] **Step 3: Commit**

```bash
git add -u desktop-ui/src/features/notes/
git commit -m "refactor(graph): delete Cytoscape hooks, theme, Cola physics, and element diff utilities"
```

---

### Task 16: Build and lint verification

**Files:** None (verification only)

- [ ] **Step 1: Run TypeScript type check**

Run:
```bash
cd desktop-ui && npx tsc --noEmit --pretty 2>&1 | tail -30
```

Expected: No errors. If there are type errors, fix them. Common issues:
- `ForceGraphMethods` type imports may need adjustment based on `react-force-graph-2d` version
- `d3Force()` accessor may need explicit typing
- `ForceNode` fields used in tooltip may need `label` → `title` aliasing

- [ ] **Step 2: Run Biome lint**

Run:
```bash
cd desktop-ui && bun run lint:fix
```

Expected: Auto-fixes applied, no remaining errors.

- [ ] **Step 3: Run build**

Run:
```bash
cd desktop-ui && bun run build
```

Expected: Successful Vite build with no errors. Bundle size should decrease (Cytoscape + Cola + fCoSE removed) or stay similar (Three.js added but only loaded for 3D mode).

- [ ] **Step 4: Run existing tests**

Run:
```bash
cd desktop-ui && bun run test
```

Expected: All existing tests pass. `graphBfs.test.ts` and `graphFingerprint.test.ts` should pass unchanged. `elementDiff.test.ts` was deleted (no longer needed).

- [ ] **Step 5: Commit any fixes**

```bash
git add desktop-ui/
git commit -m "fix(graph): resolve type errors and lint issues from migration"
```

---

### Task 17: Manual smoke test

**Files:** None (manual verification)

- [ ] **Step 1: Start the dev environment**

Run:
```bash
cd desktop-ui && bun run dev
```

Then in a separate terminal:
```bash
cargo tauri dev
```

Or just test in browser at `localhost:1420`.

- [ ] **Step 2: Verify 2D graph renders**

Check:
- Graph loads with nodes visible (glowing circles with halo)
- Link particles animate along edges
- Nodes are colored by notebook cluster
- Labels appear when zoomed in, hide when zoomed out
- Hover: node glows brighter, non-neighbors dim, tooltip shows
- Click: navigates to note
- Drag: node repositions, settles naturally
- Zoom controls (+, -, fit) work
- Re-layout button works

- [ ] **Step 3: Verify progressive reveal**

Check:
- On first load (no cache): nodes appear in BFS waves from hub
- On reload (cached): nodes appear faster with cached positions
- Try each reveal speed in settings (Instant / Balanced / Cinematic)

- [ ] **Step 4: Verify minimap**

Check:
- Minimap shows in bottom-left with colored dots
- Viewport rectangle moves when you pan/zoom
- Click on minimap navigates the main view
- Toggle button hides/shows minimap

- [ ] **Step 5: Verify toolbar and settings**

Check:
- View mode pills work (Full, Local, etc.)
- ClusteringMode "Notebook" is active, "Semantic" is disabled
- Brain View toggle exists (shows placeholder or 3D if working)
- Settings popover: sliders adjust physics, reveal speed selector works
- Legend: cluster colors match nodes, toggle hides clusters, hover highlights

- [ ] **Step 6: Verify Brain View (3D)**

Check:
- Toggle "Brain View" in toolbar
- 3D view renders with spheres
- Bloom glow visible on nodes
- Orbit controls work (drag to rotate, scroll to zoom)
- Click node works
- Toggle back to 2D works

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | Install react-force-graph + three, remove cytoscape | package.json |
| 2 | Extend GraphSettings | useGraphSettings.ts |
| 3 | Create useGraphElements (flat data model) | useGraphElements.ts |
| 4 | Create graphPainters (Canvas glow + dimming) | graphPainters.ts |
| 5 | Create useWaveReveal (BFS progressive reveal) | useWaveReveal.ts |
| 6 | Create useForceGraph (2D renderer hook) | useForceGraph.ts |
| 7 | Rewrite GraphMinimap (viewport thumbnail) | GraphMinimap.tsx |
| 8 | Update GraphLegend imports | GraphLegend.tsx |
| 9 | Update GraphToolbar (clustering + brain view) | GraphToolbar.tsx |
| 10 | Update GraphSettingsPopover (new controls) | GraphSettingsPopover.tsx |
| 11 | Rewire GraphView orchestrator | GraphView.tsx |
| 12 | Create graphMaterials (Three.js) | graphMaterials.ts |
| 13 | Create useBrainView + GraphBrainView (3D) | useBrainView.ts, GraphBrainView.tsx |
| 14 | Create useGraphTheme | useGraphTheme.ts |
| 15 | Delete old Cytoscape files | 8 files deleted |
| 16 | Build + lint verification | — |
| 17 | Manual smoke test | — |
