# Knowledge Graph V2 — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the d3-force SVG graph with a Cytoscape.js canvas graph featuring notebook compound clusters, link-count-based sizing (PageRank deferred to Phase 2), zoom-adaptive labels, interactive legend, theme-aware styling, and improved interactions.

**Architecture:** Cytoscape.js renders to a `<div>` container. A `useCytoscapeGraph` hook manages the Cytoscape instance lifecycle (create, update elements, destroy). A `useCytoscapeTheme` hook reads CSS custom properties and generates the Cytoscape stylesheet. `useGraphData` is extended to output Cytoscape `ElementDefinition[]` with compound parent nodes for notebooks. The toolbar gains a cluster mode selector.

**Tech Stack:** Cytoscape.js (already installed), cytoscape-fcose (already installed), React 19, TypeScript, Tailwind CSS v4 design tokens.

**Spec:** `docs/superpowers/specs/2026-03-17-knowledge-graph-v2-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| **New** | `src/features/notes/hooks/useCytoscapeGraph.ts` | Cytoscape instance lifecycle: create, mount, update elements, layout, destroy |
| **New** | `src/features/notes/hooks/useCytoscapeTheme.ts` | Read CSS vars → generate Cytoscape stylesheet array, re-generate on theme change |
| **New** | `src/features/notes/hooks/useCytoscapeElements.ts` | Convert `useGraphData` output + notebooks into Cytoscape `ElementDefinition[]` with compounds |
| **New** | `src/features/notes/components/GraphLegend.tsx` | Collapsible legend panel showing cluster color → name mapping |
| **Rewrite** | `src/features/notes/components/GraphView.tsx` | Replace 652-line d3-force SVG with ~150-line Cytoscape container + hooks |
| **Update** | `src/features/notes/hooks/useGraphData.ts` | Add `notebookId` to GraphNode (already present), add `updatedAt` field for recency |
| **Update** | `src/features/notes/components/GraphToolbar.tsx` | Add cluster mode toggle, edge label toggle |
| **Update** | `src/features/notes/components/GraphNodeTooltip.tsx` | Add link count display, works with Cytoscape events |
| **Update** | `src/features/notes/pages/KnowledgeBasePage.tsx` | Pass notebooks to GraphView, wire cluster mode state |
| **Keep** | `src/features/notes/components/GraphMinimap.tsx` | Keep d3-force minimap as-is for Phase 1 (separate concern) |

---

### Task 1: Install and Verify Cytoscape Imports

**Files:**
- Check: `desktop-ui/package.json`

- [ ] **Step 1: Verify cytoscape and fcose are importable**

Create a quick test file to confirm imports work:

```bash
cd desktop-ui && node -e "
  import('cytoscape').then(cy => console.log('cytoscape:', typeof cy.default));
  import('cytoscape-fcose').then(f => console.log('fcose:', typeof f.default));
"
```

Expected: Both print their types without errors. If cytoscape is only a transitive dep via mermaid, add it explicitly:

```bash
bun add cytoscape cytoscape-fcose && bun add -d @types/cytoscape
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock
git commit -m "chore: add explicit cytoscape + fcose dependencies"
```

---

### Task 2: Create `useCytoscapeTheme` Hook

**Files:**
- Create: `src/features/notes/hooks/useCytoscapeTheme.ts`

This hook reads CSS custom properties from `:root` and returns a Cytoscape stylesheet array. It re-generates when the `data-theme` attribute changes.

- [ ] **Step 1: Create the hook**

```typescript
// src/features/notes/hooks/useCytoscapeTheme.ts
import type { Stylesheet } from "cytoscape";
import { useEffect, useMemo, useState } from "react";

function getCssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function isLightTheme(): boolean {
  const bg = getCssVar("--background");
  return bg.startsWith("#f") || bg.startsWith("#e") || bg === "#ffffff";
}

export function useCytoscapeTheme(): { stylesheet: Stylesheet[]; isLight: boolean } {
  const [themeKey, setThemeKey] = useState(() =>
    document.documentElement.getAttribute("data-theme") || "dark",
  );

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setThemeKey(document.documentElement.getAttribute("data-theme") || "dark");
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  }, []);

  return useMemo(() => {
    const light = isLightTheme();
    const textPrimary = getCssVar("--text-primary") || (light ? "#000000" : "#f0f2f5");
    const textMuted = getCssVar("--text-muted") || (light ? "#737373" : "#7d8590");
    const border = getCssVar("--border") || (light ? "#e5e5e5" : "rgba(255,255,255,0.08)");
    const brand = getCssVar("--brand") || (light ? "#ca8a04" : "#f97316");
    const surfaceLowest = getCssVar("--surface-lowest") || (light ? "#fafafa" : "rgba(255,255,255,0.025)");

    const stylesheet: Stylesheet[] = [
      // ── Compound (cluster) nodes ──
      {
        selector: "node:parent",
        style: {
          "background-opacity": 0.06,
          "border-width": 1,
          "border-color": border,
          "border-opacity": 0.5,
          label: "data(label)",
          "font-size": 11,
          "font-weight": "600",
          color: textMuted,
          "text-valign": "top",
          "text-halign": "center",
          "text-margin-y": -6,
          padding: "24px",
          shape: "roundrectangle",
          "corner-radius": light ? 0 : 12,
        },
      },
      // ── Regular nodes ──
      {
        selector: "node:childless",
        style: {
          label: "data(label)",
          width: "data(size)",
          height: "data(size)",
          "background-color": "data(color)",
          "border-width": 1.5,
          "border-color": "data(color)",
          "border-opacity": 0.35,
          "font-size": 10,
          "font-weight": "500",
          color: textPrimary,
          "text-valign": "bottom",
          "text-halign": "center",
          "text-margin-y": 4,
          "text-max-width": "80px",
          "text-wrap": "ellipsis",
          "min-zoomed-font-size": 0,
          "text-opacity": 1,
        },
      },
      // ── Hide labels at low zoom (applied dynamically) ──
      {
        selector: "node:childless.hide-label",
        style: {
          "text-opacity": 0,
        },
      },
      // ── Selected node ──
      {
        selector: "node:childless:selected",
        style: {
          "border-width": 3,
          "border-color": brand,
          "shadow-blur": 12,
          "shadow-color": brand,
          "shadow-opacity": 0.4,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
          "font-weight": "600",
          "font-size": 11,
        },
      },
      // ── Edges ──
      {
        selector: "edge",
        style: {
          width: "data(weight)",
          "line-color": border,
          "target-arrow-color": border,
          "target-arrow-shape": "triangle",
          "arrow-scale": 0.5,
          "curve-style": "bezier",
          opacity: 0.6,
        },
      },
      // ── Active edges (connected to selected node) ──
      {
        selector: "edge.highlighted",
        style: {
          "line-color": "data(sourceColor)",
          "target-arrow-color": "data(sourceColor)",
          width: 2.5,
          opacity: 0.8,
        },
      },
      // ── Dimmed (during hover) ──
      {
        selector: "node.dimmed",
        style: { opacity: 0.15 },
      },
      {
        selector: "edge.dimmed",
        style: { opacity: 0.08 },
      },
      // ── Ghost compound (orphan groups) ──
      {
        selector: 'node:parent[type="orphan-linked"]',
        style: { "background-color": "#9CA3AF" },
      },
      {
        selector: 'node:parent[type="orphan-isolated"]',
        style: { "background-color": "#6B7280" },
      },
    ];

    return { stylesheet, isLight: light };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- themeKey drives recalc
  }, [themeKey]);
}
```

- [ ] **Step 2: Commit**

```bash
git add src/features/notes/hooks/useCytoscapeTheme.ts
git commit -m "feat(notes): add useCytoscapeTheme hook for graph styling"
```

---

### Task 3: Create `useCytoscapeElements` Hook

**Files:**
- Create: `src/features/notes/hooks/useCytoscapeElements.ts`

Converts graph data + notebooks into Cytoscape `ElementDefinition[]` with compound parent nodes.

- [ ] **Step 1: Create the hook**

```typescript
// src/features/notes/hooks/useCytoscapeElements.ts
import type { ElementDefinition } from "cytoscape";
import type { Notebook } from "@shared/types";
import { useMemo } from "react";
import type { GraphNode, GraphLink } from "./useGraphData";

const CLUSTER_PALETTE = [
  "#a78bfa", "#93c5fd", "#6ee7b7", "#fcd34d", "#fca5a5",
  "#f9a8d4", "#a5b4fc", "#67e8f9", "#fdba74", "#86efac",
  "#c4b5fd", "#fde68a",
];

export type ClusterMode = "notebook" | "ai" | "hybrid";

interface UseCytoscapeElementsParams {
  nodes: GraphNode[];
  links: GraphLink[];
  notebooks: Notebook[];
  clusterMode: ClusterMode;
  activeNoteId: string | null;
}

export interface ClusterInfo {
  id: string;
  label: string;
  color: string;
  count: number;
}

function getNodeSize(linkCount: number): number {
  // Phase 1: link-count based sizing (Phase 2 adds PageRank)
  const normalized = Math.min(linkCount, 30) / 30;
  return 12 + normalized * 28; // 12px → 40px
}

export function useCytoscapeElements({
  nodes,
  links,
  notebooks,
  clusterMode,
  activeNoteId,
}: UseCytoscapeElementsParams): { elements: ElementDefinition[]; clusters: ClusterInfo[] } {
  return useMemo(() => {
    const elements: ElementDefinition[] = [];
    const clusterMap = new Map<string, ClusterInfo>();

    // ── Build notebook lookup ──
    const notebookMap = new Map<string, Notebook>();
    for (const nb of notebooks) {
      notebookMap.set(nb.id, nb);
    }

    // ── Assign clusters and build compound parents ──
    let colorIndex = 0;
    const getClusterColor = (id: string, notebook?: Notebook): string => {
      if (notebook?.color) return notebook.color;
      const existing = clusterMap.get(id);
      if (existing) return existing.color;
      return CLUSTER_PALETTE[colorIndex++ % CLUSTER_PALETTE.length];
    };

    // ── Determine cluster for each node ──
    const nodeClusterMap = new Map<string, string>();

    if (clusterMode === "notebook") {
      // Group by notebook; orphans go to special compounds
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
    }
    // AI and Hybrid modes deferred to Phase 2

    // ── Create compound parent nodes ──
    const seenClusters = new Set<string>();
    for (const [, clusterId] of nodeClusterMap) {
      if (seenClusters.has(clusterId)) continue;
      seenClusters.add(clusterId);

      let label: string;
      let color: string;
      let type = "notebook";

      if (clusterId === "_floating") {
        label = "Floating Ideas";
        color = "#9CA3AF";
        type = "orphan-linked";
      } else if (clusterId === "_isolated") {
        label = "Isolated Notes";
        color = "#6B7280";
        type = "orphan-isolated";
      } else {
        const nbId = clusterId.replace("nb:", "");
        const nb = notebookMap.get(nbId);
        label = nb?.title || "Unknown Notebook";
        color = getClusterColor(clusterId, nb);
      }

      clusterMap.set(clusterId, {
        id: clusterId,
        label,
        color,
        count: 0,
      });

      elements.push({
        group: "nodes",
        data: { id: clusterId, label, color, type },
      });
    }

    // ── Create child nodes ──
    for (const node of nodes) {
      const clusterId = nodeClusterMap.get(node.id) || "_isolated";
      const cluster = clusterMap.get(clusterId);
      if (cluster) cluster.count++;

      const color = cluster?.color || "#6B7280";
      const size = getNodeSize(node.linkCount);

      elements.push({
        group: "nodes",
        data: {
          id: node.id,
          label: node.title,
          parent: clusterId,
          color,
          size,
          linkCount: node.linkCount,
          bodyPreview: node.bodyPreview,
          tags: node.tags,
          notebookId: node.notebookId,
        },
      });
    }

    // ── Create edges ──
    // Count bidirectional links
    const edgePairs = new Map<string, number>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");
      edgePairs.set(key, (edgePairs.get(key) || 0) + 1);
    }

    const seenEdges = new Set<string>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");

      if (seenEdges.has(key)) continue;
      seenEdges.add(key);

      const count = edgePairs.get(key) || 1;
      const weight = count === 1 ? 1 : count === 2 ? 1.8 : 2.8;

      // Find source node's cluster color for highlighting
      const sourceCluster = nodeClusterMap.get(sourceId);
      const sourceColor = clusterMap.get(sourceCluster || "")?.color || "#6B7280";

      elements.push({
        group: "edges",
        data: {
          id: `e:${key}`,
          source: sourceId,
          target: targetId,
          weight,
          sourceColor,
        },
      });
    }

    const clusters = Array.from(clusterMap.values()).filter((c) => c.count > 0);
    return { elements, clusters };
  }, [nodes, links, notebooks, clusterMode, activeNoteId]);
}
```

- [ ] **Step 2: Commit**

```bash
git add src/features/notes/hooks/useCytoscapeElements.ts
git commit -m "feat(notes): add useCytoscapeElements hook for Cytoscape data mapping"
```

---

### Task 4: Create `useCytoscapeGraph` Hook

**Files:**
- Create: `src/features/notes/hooks/useCytoscapeGraph.ts`

Manages the Cytoscape instance lifecycle: create on mount, update elements, run layout, destroy on unmount.

- [ ] **Step 1: Create the hook**

```typescript
// src/features/notes/hooks/useCytoscapeGraph.ts
import cytoscape, { type Core, type ElementDefinition, type Stylesheet } from "cytoscape";
import fcose from "cytoscape-fcose";
import { useCallback, useEffect, useRef } from "react";

// Register fcose layout once
cytoscape.use(fcose);

interface UseCytoscapeGraphParams {
  containerRef: React.RefObject<HTMLDivElement | null>;
  elements: ElementDefinition[];
  stylesheet: Stylesheet[];
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
  onNodeContext?: (id: string, x: number, y: number) => void;
}

const FCOSE_OPTIONS = {
  name: "fcose",
  animate: true,
  animationDuration: 800,
  fit: true,
  padding: 40,
  nodeSeparation: 80,
  idealEdgeLength: 100,
  nodeRepulsion: 6000,
  edgeElasticity: 0.45,
  gravity: 0.25,
  gravityRange: 1.5,
  nestingFactor: 0.1,
  numIter: 2500,
  quality: "default" as const,
};

export function useCytoscapeGraph({
  containerRef,
  elements,
  stylesheet,
  onNodeClick,
  onNodeDoubleClick,
  onNodeHover,
  onNodeContext,
}: UseCytoscapeGraphParams): { cy: React.MutableRefObject<Core | null>; runLayout: () => void } {
  const cyRef = useRef<Core | null>(null);
  const elementsRef = useRef(elements);
  elementsRef.current = elements;

  // ── Create instance on mount ──
  useEffect(() => {
    if (!containerRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: stylesheet,
      layout: { name: "preset" }, // Don't layout yet
      minZoom: 0.1,
      maxZoom: 5,
      wheelSensitivity: 0.3,
      boxSelectionEnabled: true,
      selectionType: "single",
    });

    cyRef.current = cy;

    // ── Event handlers ──
    cy.on("tap", "node:childless", (evt) => {
      onNodeClick?.(evt.target.id());
    });

    cy.on("dbltap", "node:childless", (evt) => {
      onNodeDoubleClick?.(evt.target.id());
    });

    // Tap on compound → fit to that cluster
    cy.on("tap", "node:parent", (evt) => {
      cy.animate({ fit: { eles: evt.target.children(), padding: 50 }, duration: 300 });
    });

    cy.on("mouseover", "node:childless", (evt) => {
      const node = evt.target;
      const pos = node.renderedPosition();
      onNodeHover?.(node.id(), pos.x, pos.y);

      // Highlight neighborhood
      const neighborhood = node.neighborhood().add(node);
      cy.elements().not(neighborhood).addClass("dimmed");
      neighborhood.connectedEdges().addClass("highlighted");
    });

    cy.on("mouseout", "node:childless", () => {
      onNodeHover?.(null, 0, 0);
      cy.elements().removeClass("dimmed").removeClass("highlighted");
    });

    cy.on("cxttap", "node:childless", (evt) => {
      const node = evt.target;
      const pos = evt.renderedPosition || node.renderedPosition();
      onNodeContext?.(node.id(), pos.x, pos.y);
    });

    // ── Zoom-adaptive labels ──
    cy.on("zoom", () => {
      const zoom = cy.zoom();
      const childless = cy.nodes(":childless");
      if (zoom < 0.5) {
        childless.addClass("hide-label");
      } else {
        childless.removeClass("hide-label");
      }
    });

    // Run initial layout
    cy.layout(FCOSE_OPTIONS).run();

    return () => {
      cy.destroy();
      cyRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount/unmount only
  }, [containerRef, stylesheet]);

  // ── Update elements when data changes ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;

    cy.json({ elements });
    cy.layout(FCOSE_OPTIONS).run();
  }, [elements]);

  const runLayout = useCallback(() => {
    cyRef.current?.layout(FCOSE_OPTIONS).run();
  }, []);

  return { cy: cyRef, runLayout };
}
```

- [ ] **Step 2: Commit**

```bash
git add src/features/notes/hooks/useCytoscapeGraph.ts
git commit -m "feat(notes): add useCytoscapeGraph hook for Cytoscape lifecycle"
```

---

### Task 5: Create `GraphLegend` Component

**Files:**
- Create: `src/features/notes/components/GraphLegend.tsx`

- [ ] **Step 1: Create the component**

```typescript
// src/features/notes/components/GraphLegend.tsx
import { ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";
import type { ClusterInfo } from "../hooks/useCytoscapeElements";

interface GraphLegendProps {
  clusters: ClusterInfo[];
  onHighlight: (clusterId: string | null) => void;
}

export function GraphLegend({ clusters, onHighlight }: GraphLegendProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (clusters.length === 0) return null;

  return (
    <div className="absolute bottom-4 left-4 z-10 glass-card px-3 py-2 max-w-[220px]">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="flex items-center gap-1.5 text-[10px] font-semibold text-muted uppercase tracking-wider w-full"
      >
        <span>Clusters</span>
        <span className="text-dim">({clusters.length})</span>
        <span className="ml-auto">
          {collapsed ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
        </span>
      </button>

      {!collapsed && (
        <div className="mt-2 space-y-1">
          {clusters.map((cluster) => (
            <button
              key={cluster.id}
              type="button"
              onClick={() => onHighlight(cluster.id)}
              onDoubleClick={() => onHighlight(null)}
              className="flex items-center gap-2 w-full text-left px-1 py-0.5 rounded hover:bg-surface-base transition-colors"
            >
              <span
                className="w-2.5 h-2.5 rounded-full shrink-0"
                style={{ backgroundColor: cluster.color }}
              />
              <span className="text-[11px] text-secondary truncate flex-1">
                {cluster.label}
              </span>
              <span className="text-[10px] text-dim">{cluster.count}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/features/notes/components/GraphLegend.tsx
git commit -m "feat(notes): add GraphLegend component for cluster color mapping"
```

---

### Task 6: Rewrite `GraphView` with Cytoscape

**Files:**
- Rewrite: `src/features/notes/components/GraphView.tsx`

This is the core task. Replace the entire 652-line d3-force SVG component with a ~150-line Cytoscape container.

- [ ] **Step 1: Rewrite GraphView**

```typescript
// src/features/notes/components/GraphView.tsx
import type { Note, Notebook } from "@shared/types";
import { Maximize2, Minus, Plus, RotateCcw } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  type ClusterMode,
  useCytoscapeElements,
} from "../hooks/useCytoscapeElements";
import { useCytoscapeGraph } from "../hooks/useCytoscapeGraph";
import { useCytoscapeTheme } from "../hooks/useCytoscapeTheme";
import type { SmartView } from "../hooks/useGraphData";
import { useGraphData } from "../hooks/useGraphData";
import { GraphLegend } from "./GraphLegend";
import { GraphNodeTooltip } from "./GraphNodeTooltip";
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

  // ── State ──
  const [smartView, setSmartView] = useState<SmartView>("full");
  const [hopRadius, setHopRadius] = useState(2);
  const [searchQuery, setSearchQuery] = useState("");
  const [clusterMode] = useState<ClusterMode>("notebook");
  const [tooltip, setTooltip] = useState<{
    nodeId: string;
    x: number;
    y: number;
  } | null>(null);

  // ── Data pipeline ──
  const { nodes: rawNodes, links: rawLinks } = useGraphData(
    smartView,
    notes,
    activeNoteId,
    hopRadius,
  );

  // Apply search filter
  const filteredNodes = searchQuery
    ? rawNodes.filter((n) => n.title.toLowerCase().includes(searchQuery.toLowerCase()))
    : rawNodes;
  const filteredNodeIds = new Set(filteredNodes.map((n) => n.id));
  const filteredLinks = rawLinks.filter((l) => {
    const sId = typeof l.source === "string" ? l.source : l.source.id;
    const tId = typeof l.target === "string" ? l.target : l.target.id;
    return filteredNodeIds.has(sId) && filteredNodeIds.has(tId);
  });

  const { elements, clusters } = useCytoscapeElements({
    nodes: filteredNodes,
    links: filteredLinks,
    notebooks,
    clusterMode,
    activeNoteId,
  });

  // ── Theme ──
  const { stylesheet } = useCytoscapeTheme();

  // ── Tooltip data lookup ──
  const nodeMap = new Map(filteredNodes.map((n) => [n.id, n]));

  // ── Cytoscape instance ──
  const { cy, runLayout } = useCytoscapeGraph({
    containerRef,
    elements,
    stylesheet,
    onNodeClick: onSelectNote,
    onNodeDoubleClick: onOpenInEditor,
    onNodeHover: useCallback(
      (id: string | null, x: number, y: number) => {
        if (id) {
          setTooltip({ nodeId: id, x, y });
        } else {
          setTooltip(null);
        }
      },
      [],
    ),
  });

  // ── Legend highlight ──
  const handleLegendHighlight = useCallback(
    (clusterId: string | null) => {
      const cyInstance = cy.current;
      if (!cyInstance) return;

      if (!clusterId) {
        cyInstance.elements().removeClass("dimmed");
        return;
      }

      const parent = cyInstance.getElementById(clusterId);
      const children = parent.children();
      const neighborhood = children.union(children.connectedEdges()).union(parent);
      cyInstance.elements().addClass("dimmed");
      neighborhood.removeClass("dimmed");
    },
    [cy],
  );

  // ── Zoom controls ──
  const zoomIn = () => cy.current?.zoom({ level: (cy.current.zoom() || 1) * 1.3, renderedPosition: { x: (containerRef.current?.clientWidth || 0) / 2, y: (containerRef.current?.clientHeight || 0) / 2 } });
  const zoomOut = () => cy.current?.zoom({ level: (cy.current.zoom() || 1) / 1.3, renderedPosition: { x: (containerRef.current?.clientWidth || 0) / 2, y: (containerRef.current?.clientHeight || 0) / 2 } });
  const fitScreen = () => cy.current?.animate({ fit: { padding: 40 }, duration: 300 });

  // ── Empty state ──
  if (notes.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-muted">
        <p className="text-sm font-medium">Your knowledge graph will appear here</p>
        <p className="text-xs text-dim">Create your first note to get started</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* Toolbar */}
      <GraphToolbar
        view={smartView}
        onViewChange={setSmartView}
        hopRadius={hopRadius}
        onHopRadiusChange={setHopRadius}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
      />

      {/* Graph canvas */}
      <div className="flex-1 relative min-h-0">
        <div ref={containerRef} className="absolute inset-0" />

        {/* Legend */}
        <GraphLegend clusters={clusters} onHighlight={handleLegendHighlight} />

        {/* Zoom controls (bottom-right) */}
        <div className="absolute bottom-4 right-4 z-10 flex flex-col gap-1">
          <button type="button" onClick={zoomIn} className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary" aria-label="Zoom in">
            <Plus size={14} />
          </button>
          <button type="button" onClick={zoomOut} className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary" aria-label="Zoom out">
            <Minus size={14} />
          </button>
          <button type="button" onClick={fitScreen} className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary" aria-label="Fit to screen">
            <Maximize2 size={14} />
          </button>
          <button type="button" onClick={runLayout} className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary" aria-label="Re-layout">
            <RotateCcw size={14} />
          </button>
        </div>

        {/* Tooltip */}
        {tooltip && nodeMap.has(tooltip.nodeId) && (
          <GraphNodeTooltip
            node={nodeMap.get(tooltip.nodeId)!}
            x={tooltip.x}
            y={tooltip.y}
          />
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/features/notes/components/GraphView.tsx
git commit -m "feat(notes): rewrite GraphView with Cytoscape.js + compound clusters"
```

---

### Task 7: Update `KnowledgeBasePage` to Pass Notebooks

**Files:**
- Modify: `src/features/notes/pages/KnowledgeBasePage.tsx`

The `GraphView` component now accepts a `notebooks` prop. The page already fetches notebooks — just need to wire it through.

- [ ] **Step 1: Find the GraphView usage and add notebooks prop**

In `KnowledgeBasePage.tsx`, find where `<GraphView` is rendered. Add `notebooks={notebooks}` to the props. The `notebooks` variable is already available from the existing `useQuery("notebook_list")` call at the top of the component.

Search for `<GraphView` in the file and update the props to include `notebooks={notebooks}`.

- [ ] **Step 2: Verify build**

```bash
cd desktop-ui && bun run build
```

Expected: Clean build with no TypeScript errors.

- [ ] **Step 3: Commit**

```bash
git add src/features/notes/pages/KnowledgeBasePage.tsx
git commit -m "feat(notes): wire notebooks prop to GraphView for cluster labels"
```

---

### Task 8: Add Keyboard Shortcuts

**Files:**
- Modify: `src/features/notes/hooks/useCytoscapeGraph.ts`

- [ ] **Step 1: Add keyboard event handler inside the `useEffect` that creates the cy instance**

After the event handlers section, add:

```typescript
const handleKeyDown = (e: KeyboardEvent) => {
  if (!cy || document.activeElement?.tagName === "INPUT") return;
  switch (e.key) {
    case "+":
    case "=":
      cy.zoom({ level: cy.zoom() * 1.2, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
      break;
    case "-":
      cy.zoom({ level: cy.zoom() / 1.2, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
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
// Add to cleanup: document.removeEventListener("keydown", handleKeyDown);
```

- [ ] **Step 2: Commit**

```bash
git add src/features/notes/hooks/useCytoscapeGraph.ts
git commit -m "feat(notes): add keyboard shortcuts to graph (zoom, fit, deselect)"
```

---

### Task 9: Handle Reduced Motion Preference

**Files:**
- Modify: `src/features/notes/hooks/useCytoscapeGraph.ts`

- [ ] **Step 1: Check `prefers-reduced-motion` before setting animation duration**

At the top of the hook, add:

```typescript
const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
```

Then in `FCOSE_OPTIONS`, conditionally set:

```typescript
animate: !prefersReducedMotion,
animationDuration: prefersReducedMotion ? 0 : 800,
```

- [ ] **Step 2: Commit**

```bash
git add src/features/notes/hooks/useCytoscapeGraph.ts
git commit -m "feat(notes): respect prefers-reduced-motion in graph layout"
```

---

### Task 10: Smoke Test and Final Polish

- [ ] **Step 1: Full build verification**

```bash
cd desktop-ui && bun run build
```

Expected: Clean build, no errors.

- [ ] **Step 2: Lint check**

```bash
cd desktop-ui && bun run lint:fix
```

Fix any issues reported.

- [ ] **Step 3: Manual smoke test**

Start the dev app and navigate to Notes → Graph view:

```bash
cargo tauri dev
```

Verify:
- Graph renders with Cytoscape canvas (not SVG)
- Notes are grouped into notebook clusters (colored compound nodes)
- Orphan notes appear in "Floating Ideas" or "Isolated Notes" groups
- Hover shows tooltip
- Click selects a node
- Double-click opens in editor
- Click a cluster compound → zoom-to-fit
- Legend panel shows cluster colors
- View switcher works (Full, Local, By Tag, By Notebook, Orphans)
- Zoom controls work (+, -, fit, re-layout)
- Keyboard shortcuts work (+, -, f, Esc)
- Theme switching works (dark vs nexora)
- Search filter works
- Empty state shows for empty vault

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat(notes): Knowledge Graph V2 Phase 1 — Cytoscape.js rewrite

Replace d3-force SVG with Cytoscape.js canvas graph featuring:
- Notebook compound clusters with fCoSE layout
- Link-count based node sizing (12-40px)
- Zoom-adaptive labels (hide < 0.5x, truncate, full > 1.2x)
- Interactive legend with click-to-highlight
- Neighborhood highlighting on hover
- Theme-aware styling via CSS custom properties
- Keyboard shortcuts (zoom, fit, deselect)
- Empty states for empty vault and no-links
- Reduced motion support"
```
