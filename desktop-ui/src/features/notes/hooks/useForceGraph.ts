import { forceCollide } from "d3-force";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ForceGraphMethods } from "react-force-graph-2d";
import { type PaintContext, paintLink, paintNode } from "../lib/graphPainters";
import type { ForceLink, ForceNode, GraphElements } from "./useGraphElements";
import type { PositionMap } from "./useGraphPositionCache";
import type { GraphSettings } from "./useGraphSettings";

// ── Exported types ──────────────────────────────────────────────────────

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

// ── Hook params ─────────────────────────────────────────────────────────

interface UseForceGraphParams {
  elements: GraphElements;
  settings: GraphSettings;
  renderMode: "2d" | "3d";
  activeNoteId: string | null;
  highlightedClusterId: string | null;
  revealedNodes: Set<string>;
  cachedPositions?: PositionMap | null;
  onNodeClick: (nodeId: string) => void;
  onNodeDoubleClick?: (nodeId: string) => void;
  onSavePositions: (positions: PositionMap) => void;
  onNudge?: (nudge: GraphNudge) => void;
}

// ── Hook return ─────────────────────────────────────────────────────────

export interface ForceGraphController {
  graphRef: React.MutableRefObject<ForceGraphMethods | undefined>;
  graphData: { nodes: ForceNode[]; links: ForceLink[] };
  nodeCanvasObject: (node: ForceNode, ctx: CanvasRenderingContext2D, globalScale: number) => void;
  nodeCanvasObjectMode: () => "replace";
  linkCanvasObject: (link: ForceLink, ctx: CanvasRenderingContext2D, globalScale: number) => void;
  linkCanvasObjectMode: () => "replace";
  nodePointerAreaPaint: (
    node: ForceNode,
    color: string,
    ctx: CanvasRenderingContext2D,
    globalScale: number,
  ) => void;
  onNodeClick: (node: ForceNode, event: MouseEvent) => void;
  onNodeHover: (node: ForceNode | null) => void;
  onNodeDragEnd: (node: ForceNode) => void;
  onBackgroundClick: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
  fitToScreen: () => void;
  getViewportBounds: () => ViewportBounds;
  snapshotPositions: () => PositionMap;
  configureForces: () => void;
  hoveredNodeId: string | null;
}

// ── Constants ───────────────────────────────────────────────────────────

const CLUSTER_ATTRACTION = 0.03;
const ZOOM_DURATION = 300;
const ZOOM_FACTOR = 1.4;

// ── Hook implementation ─────────────────────────────────────────────────

export function useForceGraph({
  elements,
  settings,
  renderMode,
  activeNoteId: _activeNoteId,
  highlightedClusterId,
  revealedNodes,
  cachedPositions,
  onNodeClick,
  onNodeDoubleClick,
  onSavePositions,
  onNudge,
}: UseForceGraphParams): ForceGraphController {
  const graphRef = useRef<ForceGraphMethods>(undefined);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const neighborSetRef = useRef<Set<string>>(new Set());
  const lastClickRef = useRef<{ nodeId: string; time: number } | null>(null);

  // Build adjacency for neighbor lookup
  const adjacencyRef = useRef<Map<string, Set<string>>>(new Map());
  useEffect(() => {
    const adj = new Map<string, Set<string>>();
    for (const link of elements.links) {
      const sId =
        typeof link.source === "string" ? link.source : (link.source as never as ForceNode).id;
      const tId =
        typeof link.target === "string" ? link.target : (link.target as never as ForceNode).id;
      if (!adj.has(sId)) adj.set(sId, new Set());
      if (!adj.has(tId)) adj.set(tId, new Set());
      adj.get(sId)?.add(tId);
      adj.get(tId)?.add(sId);
    }
    adjacencyRef.current = adj;
  }, [elements.links]);

  // Stable graph data ref — react-force-graph mutates node objects (adds x, y, vx, vy).
  // We must NOT recreate node objects on re-render or the simulation restarts.
  // Only update when the set of node/link IDs actually changes.
  const graphDataRef = useRef<{ nodes: ForceNode[]; links: ForceLink[] }>({ nodes: [], links: [] });
  const prevFingerprintRef = useRef<string>("");

  // Reset stale node refs when switching render mode —
  // 3D simulation mutates shared node objects with 3D-scale positions,
  // and we need to re-initialize forces for the new ForceGraph2D instance
  const prevRenderModeRef = useRef(renderMode);
  const forceInitializedRef = useRef(false);
  if (prevRenderModeRef.current !== renderMode) {
    prevRenderModeRef.current = renderMode;
    graphDataRef.current = { nodes: [], links: [] };
    prevFingerprintRef.current = "";
    forceInitializedRef.current = false;
  }

  const graphData = useMemo(() => {
    // Build a fingerprint of node IDs + link source/targets to detect structural changes
    const nodeIds = elements.nodes.map((n) => n.id).sort().join(",");
    const linkIds = elements.links
      .map((l) => `${typeof l.source === "string" ? l.source : (l.source as never as ForceNode).id}-${typeof l.target === "string" ? l.target : (l.target as never as ForceNode).id}`)
      .sort()
      .join(",");
    const fingerprint = `${nodeIds}|${linkIds}`;

    if (fingerprint === prevFingerprintRef.current) {
      // Structure unchanged — return the SAME object ref so ForceGraph2D doesn't restart
      return graphDataRef.current;
    }

    prevFingerprintRef.current = fingerprint;

    // Structure changed — build new graph data, applying cached positions
    const nodes = elements.nodes.map((node) => {
      // Check if this node already exists in the previous data (preserve simulation state)
      const existing = graphDataRef.current.nodes.find((n) => n.id === node.id);
      if (existing) {
        // Update data fields but keep simulation-managed x/y/vx/vy
        Object.assign(existing, {
          label: node.label,
          color: node.color,
          size: node.size,
          linkCount: node.linkCount,
          tags: node.tags,
          bodyPreview: node.bodyPreview,
          notebookId: node.notebookId,
          clusterId: node.clusterId,
        });
        return existing;
      }
      // New node — apply cached position if available
      const cached = cachedPositions?.[node.id];
      if (cached) {
        return { ...node, x: cached.x, y: cached.y };
      }
      return { ...node };
    });

    const data = { nodes, links: [...elements.links] };
    graphDataRef.current = data;
    return data;
  }, [elements, cachedPositions]);

  // ── Force configuration ─────────────────────────────────────────────

  const configureForces = useCallback(
    (reheat = false) => {
      const fg = graphRef.current;
      if (!fg) return;

      // Charge (repulsion)
      const charge = fg.d3Force("charge");
      if (charge && "strength" in charge) {
        (charge as unknown as { strength: (v: number) => void }).strength(-settings.repulsion);
      }

      // Center
      const center = fg.d3Force("center");
      if (center && "strength" in center) {
        (center as unknown as { strength: (v: number) => void }).strength(settings.centerForce);
      }

      // Link distance
      const link = fg.d3Force("link");
      if (link && "distance" in link) {
        (link as unknown as { distance: (v: number) => void }).distance(settings.linkDistance);
      }

      // Collide
      fg.d3Force(
        "collide",
        forceCollide<ForceNode>((node) => (node.size / 2) * settings.nodeScale + 4) as never,
      );

      // Cluster attraction: pull nodes toward cluster centroid
      fg.d3Force("clusterAttraction", clusterAttractionForce(CLUSTER_ATTRACTION) as never);

      if (reheat) fg.d3ReheatSimulation();
    },
    [settings.repulsion, settings.centerForce, settings.linkDistance, settings.nodeScale],
  );

  // Configure forces on mount or after render mode switch, then zoom to fit
  // biome-ignore lint/correctness/useExhaustiveDependencies: renderMode triggers re-init
  useEffect(() => {
    if (forceInitializedRef.current) return;
    const timer = setTimeout(() => {
      configureForces(true);
      forceInitializedRef.current = true;
      // Zoom to fit after simulation settles
      setTimeout(() => {
        graphRef.current?.zoomToFit(400, 60);
      }, 2000);
    }, 100);
    return () => clearTimeout(timer);
  }, [configureForces, renderMode]);

  // Reheat only when physics settings actually change (after initial mount)
  const prevPhysicsRef = useRef({
    repulsion: settings.repulsion,
    centerForce: settings.centerForce,
    linkDistance: settings.linkDistance,
  });
  useEffect(() => {
    const prev = prevPhysicsRef.current;
    if (
      prev.repulsion !== settings.repulsion ||
      prev.centerForce !== settings.centerForce ||
      prev.linkDistance !== settings.linkDistance
    ) {
      prevPhysicsRef.current = {
        repulsion: settings.repulsion,
        centerForce: settings.centerForce,
        linkDistance: settings.linkDistance,
      };
      configureForces(true);
    }
  }, [settings.repulsion, settings.centerForce, settings.linkDistance, configureForces]);

  // ── Canvas painting ─────────────────────────────────────────────────

  const nodeCanvasObject = useCallback(
    (node: ForceNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
      if (revealedNodes.size > 0 && !revealedNodes.has(node.id)) return;

      const paintCtx: PaintContext = {
        nodeScale: settings.nodeScale,
        labelThreshold: settings.labelThreshold,
        hoveredNodeId,
        neighborSet: neighborSetRef.current,
        highlightedClusterId,
      };
      paintNode(node, ctx, globalScale, paintCtx);
    },
    [
      settings.nodeScale,
      settings.labelThreshold,
      hoveredNodeId,
      highlightedClusterId,
      revealedNodes,
    ],
  );

  const nodeCanvasObjectMode = useCallback(() => "replace" as const, []);

  const linkCanvasObject = useCallback(
    (link: ForceLink, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const source = link.source as unknown as ForceNode;
      const target = link.target as unknown as ForceNode;
      if (
        revealedNodes.size > 0 &&
        (!revealedNodes.has(source.id) || !revealedNodes.has(target.id))
      ) {
        return;
      }

      const paintCtx: PaintContext = {
        nodeScale: settings.nodeScale,
        labelThreshold: settings.labelThreshold,
        hoveredNodeId,
        neighborSet: neighborSetRef.current,
        highlightedClusterId,
      };
      paintLink(link, ctx, globalScale, paintCtx);
    },
    [
      settings.nodeScale,
      settings.labelThreshold,
      hoveredNodeId,
      highlightedClusterId,
      revealedNodes,
    ],
  );

  const linkCanvasObjectMode = useCallback(() => "replace" as const, []);

  // Pointer area for hit detection
  const nodePointerAreaPaint = useCallback(
    (node: ForceNode, color: string, ctx: CanvasRenderingContext2D, _globalScale: number) => {
      const x = node.x ?? 0;
      const y = node.y ?? 0;
      const radius = (node.size / 2) * settings.nodeScale;
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(x, y, radius + 4, 0, Math.PI * 2);
      ctx.fill();
    },
    [settings.nodeScale],
  );

  // ── Interaction handlers ────────────────────────────────────────────

  const handleNodeClick = useCallback(
    (node: ForceNode, _event: MouseEvent) => {
      const now = Date.now();
      if (
        lastClickRef.current &&
        lastClickRef.current.nodeId === node.id &&
        now - lastClickRef.current.time < 400
      ) {
        // Double-click
        onNodeDoubleClick?.(node.id);
        lastClickRef.current = null;
        return;
      }
      lastClickRef.current = { nodeId: node.id, time: now };
      onNodeClick(node.id);
    },
    [onNodeClick, onNodeDoubleClick],
  );

  const handleNodeHover = useCallback((node: ForceNode | null) => {
    if (node) {
      setHoveredNodeId(node.id);
      neighborSetRef.current = adjacencyRef.current.get(node.id) ?? new Set();
    } else {
      setHoveredNodeId(null);
      neighborSetRef.current = new Set();
    }
  }, []);

  // ── Position snapshot (must be defined before handleNodeDragEnd) ───

  const snapshotCurrentPositions = useCallback((): PositionMap => {
    const positions: PositionMap = {};
    for (const node of elements.nodes) {
      if (node.x != null && node.y != null) {
        positions[node.id] = { x: node.x, y: node.y };
      }
    }
    return positions;
  }, [elements.nodes]);

  const handleNodeDragEnd = useCallback(
    (node: ForceNode) => {
      // Pin node at dragged position
      node.fx = node.x;
      node.fy = node.y;

      // Emit nudge event
      if (onNudge && node.x != null && node.y != null) {
        onNudge({
          nodeId: node.id,
          position: { x: node.x, y: node.y },
          clusterId: node.clusterId,
          timestamp: Date.now(),
        });
      }

      // Save positions
      const positions = snapshotCurrentPositions();
      onSavePositions(positions);
    },
    [onNudge, onSavePositions, snapshotCurrentPositions],
  );

  const handleBackgroundClick = useCallback(() => {
    setHoveredNodeId(null);
    neighborSetRef.current = new Set();
  }, []);

  // ── Zoom/pan controls ───────────────────────────────────────────────

  const zoomIn = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;
    const currentZoom = fg.zoom();
    fg.zoom(currentZoom * ZOOM_FACTOR, ZOOM_DURATION);
  }, []);

  const zoomOut = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;
    const currentZoom = fg.zoom();
    fg.zoom(currentZoom / ZOOM_FACTOR, ZOOM_DURATION);
  }, []);

  const fitToScreen = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;
    fg.zoomToFit(ZOOM_DURATION, 40);
  }, []);

  // ── Viewport bounds (for minimap) ───────────────────────────────────

  const getViewportBounds = useCallback((): ViewportBounds => {
    const fg = graphRef.current;
    if (!fg) return { x: 0, y: 0, width: 100, height: 100 };

    const topLeft = fg.screen2GraphCoords(0, 0);
    // Use a reasonable default viewport size
    const canvas = document.querySelector("canvas");
    const w = canvas?.width ?? 800;
    const h = canvas?.height ?? 600;
    const bottomRight = fg.screen2GraphCoords(w, h);

    return {
      x: topLeft.x,
      y: topLeft.y,
      width: bottomRight.x - topLeft.x,
      height: bottomRight.y - topLeft.y,
    };
  }, []);

  return {
    graphRef,
    graphData,
    nodeCanvasObject,
    nodeCanvasObjectMode,
    linkCanvasObject,
    linkCanvasObjectMode,
    nodePointerAreaPaint,
    onNodeClick: handleNodeClick,
    onNodeHover: handleNodeHover,
    onNodeDragEnd: handleNodeDragEnd,
    onBackgroundClick: handleBackgroundClick,
    zoomIn,
    zoomOut,
    fitToScreen,
    getViewportBounds,
    snapshotPositions: snapshotCurrentPositions,
    configureForces,
    hoveredNodeId,
  };
}

// ── Custom d3 force: cluster attraction ─────────────────────────────────

function clusterAttractionForce(strength: number) {
  let nodes: ForceNode[] = [];

  function force(alpha: number) {
    // Compute cluster centroids
    const centroids = new Map<string, { x: number; y: number; count: number }>();
    for (const node of nodes) {
      if (node.x == null || node.y == null) continue;
      const existing = centroids.get(node.clusterId);
      if (existing) {
        existing.x += node.x;
        existing.y += node.y;
        existing.count++;
      } else {
        centroids.set(node.clusterId, { x: node.x, y: node.y, count: 1 });
      }
    }

    // Average
    for (const c of centroids.values()) {
      c.x /= c.count;
      c.y /= c.count;
    }

    // Pull nodes toward their cluster centroid
    for (const node of nodes) {
      if (node.x == null || node.y == null) continue;
      const centroid = centroids.get(node.clusterId);
      if (!centroid || centroid.count <= 1) continue;

      const dx = centroid.x - node.x;
      const dy = centroid.y - node.y;
      node.vx = (node.vx ?? 0) + dx * strength * alpha;
      node.vy = (node.vy ?? 0) + dy * strength * alpha;
    }
  }

  force.initialize = (initNodes: ForceNode[]) => {
    nodes = initNodes;
  };

  return force;
}
