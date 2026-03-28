import { forceCollide } from "d3-force";
import { useCallback, useEffect, useRef, useState } from "react";
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
  activeNoteId,
  highlightedClusterId,
  revealedNodes,
  cachedPositions,
  onNodeClick,
  onNodeDoubleClick,
  onSavePositions,
  onNudge,
}: UseForceGraphParams): ForceGraphController {
  const graphRef = useRef<ForceGraphMethods>();
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const neighborSetRef = useRef<Set<string>>(new Set());
  const lastClickRef = useRef<{ nodeId: string; time: number } | null>(null);

  // Build adjacency for neighbor lookup
  const adjacencyRef = useRef<Map<string, Set<string>>>(new Map());
  useEffect(() => {
    const adj = new Map<string, Set<string>>();
    for (const link of elements.links) {
      const sId = typeof link.source === "string" ? link.source : (link.source as never as ForceNode).id;
      const tId = typeof link.target === "string" ? link.target : (link.target as never as ForceNode).id;
      if (!adj.has(sId)) adj.set(sId, new Set());
      if (!adj.has(tId)) adj.set(tId, new Set());
      adj.get(sId)?.add(tId);
      adj.get(tId)?.add(sId);
    }
    adjacencyRef.current = adj;
  }, [elements.links]);

  // Apply cached positions to nodes
  const graphData = useCallback(() => {
    const nodes = elements.nodes.map((node) => {
      const cached = cachedPositions?.[node.id];
      if (cached) {
        return { ...node, x: cached.x, y: cached.y, fx: undefined, fy: undefined };
      }
      return node;
    });
    return { nodes, links: [...elements.links] };
  }, [elements, cachedPositions])();

  // ── Force configuration ─────────────────────────────────────────────

  const configureForces = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;

    // Charge (repulsion)
    const charge = fg.d3Force("charge");
    if (charge && "strength" in charge) {
      (charge as { strength: (v: number) => void }).strength(-settings.repulsion);
    }

    // Center
    const center = fg.d3Force("center");
    if (center && "strength" in center) {
      (center as { strength: (v: number) => void }).strength(settings.centerForce);
    }

    // Link distance
    const link = fg.d3Force("link");
    if (link && "distance" in link) {
      (link as { distance: (v: number) => typeof link }).distance(settings.linkDistance);
    }

    // Collide
    fg.d3Force(
      "collide",
      forceCollide<ForceNode>((node) => (node.size / 2) * settings.nodeScale + 4) as never,
    );

    // Cluster attraction: pull nodes toward cluster centroid
    fg.d3Force("clusterAttraction", clusterAttractionForce(CLUSTER_ATTRACTION) as never);

    fg.d3ReheatSimulation();
  }, [settings]);

  // Reconfigure forces when settings change
  useEffect(() => {
    // Small delay to ensure the graph ref is populated
    const timer = setTimeout(configureForces, 50);
    return () => clearTimeout(timer);
  }, [configureForces]);

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
    [settings.nodeScale, settings.labelThreshold, hoveredNodeId, highlightedClusterId, revealedNodes],
  );

  const nodeCanvasObjectMode = useCallback(() => "replace" as const, []);

  const linkCanvasObject = useCallback(
    (link: ForceLink, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const source = link.source as unknown as ForceNode;
      const target = link.target as unknown as ForceNode;
      if (revealedNodes.size > 0 && (!revealedNodes.has(source.id) || !revealedNodes.has(target.id))) {
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
    [settings.nodeScale, settings.labelThreshold, hoveredNodeId, highlightedClusterId, revealedNodes],
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

  const handleNodeHover = useCallback(
    (node: ForceNode | null) => {
      if (node) {
        setHoveredNodeId(node.id);
        neighborSetRef.current = adjacencyRef.current.get(node.id) ?? new Set();
      } else {
        setHoveredNodeId(null);
        neighborSetRef.current = new Set();
      }
    },
    [],
  );

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
    [onNudge, onSavePositions],
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

  // ── Position snapshot ───────────────────────────────────────────────

  const snapshotCurrentPositions = useCallback((): PositionMap => {
    const positions: PositionMap = {};
    for (const node of elements.nodes) {
      if (node.x != null && node.y != null) {
        positions[node.id] = { x: node.x, y: node.y };
      }
    }
    return positions;
  }, [elements.nodes]);

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
