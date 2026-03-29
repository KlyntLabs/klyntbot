import { forceCollide } from "d3-force";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ForceGraphMethods } from "react-force-graph-2d";
import { convexHull, expandHull, pointInPolygon } from "../lib/convexHull";
import { type PaintContext, paintLink, paintNode } from "../lib/graphPainters";
import { resolveLinkEndpointId } from "../lib/graphUtils";
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

export interface DragCommunityChange {
  nodeId: string;
  nodeLabel: string;
  sourceCommunityId: string;
  targetCommunityId: string;
  targetCommunityName: string;
}

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
  onNodeRightClick?: (node: ForceNode, screenPos: { x: number; y: number }) => void;
  onSavePositions: (positions: PositionMap) => void;
  onNudge?: (nudge: GraphNudge) => void;
  onTreeExpand?: (noteId: string) => void;
  onTreeCollapse?: (noteId: string) => void;
  onDragCommunityChange?: (change: DragCommunityChange) => void;
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
  onNodeRightClick: (node: ForceNode, event: MouseEvent) => void;
  onNodeHover: (node: ForceNode | null) => void;
  onNodeDrag: (node: ForceNode) => void;
  onNodeDragEnd: (node: ForceNode) => void;
  onBackgroundClick: () => void;
  onEngineStop: () => void;
  onRenderFramePost: (ctx: CanvasRenderingContext2D, globalScale: number) => void;
  zoomIn: () => void;
  zoomOut: () => void;
  fitToScreen: () => void;
  getViewportBounds: () => ViewportBounds;
  snapshotPositions: () => PositionMap;
  configureForces: () => void;
  hoveredNodeId: string | null;
  highlightedNodeIds: Set<string>;
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
  activeNoteId,
  highlightedClusterId,
  revealedNodes,
  cachedPositions,
  onNodeClick,
  onNodeDoubleClick,
  onNodeRightClick,
  onSavePositions,
  onNudge,
  onTreeExpand,
  onTreeCollapse,
  onDragCommunityChange,
}: UseForceGraphParams): ForceGraphController {
  const graphRef = useRef<ForceGraphMethods>(undefined);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [highlightedNodeIds, setHighlightedNodeIds] = useState<Set<string>>(new Set());
  const neighborSetRef = useRef<Set<string>>(new Set());
  const lastClickRef = useRef<{ nodeId: string; time: number } | null>(null);

  // Build adjacency for neighbor lookup
  const adjacencyRef = useRef<Map<string, Set<string>>>(new Map());
  useEffect(() => {
    const adj = new Map<string, Set<string>>();
    for (const link of elements.links) {
      const sId = resolveLinkEndpointId(link.source as string | ForceNode);
      const tId = resolveLinkEndpointId(link.target as string | ForceNode);
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
    const nodeIds = elements.nodes
      .map((n) => n.id)
      .sort()
      .join(",");
    const linkIds = elements.links
      .map(
        (l) =>
          `${resolveLinkEndpointId(l.source as string | ForceNode)}-${resolveLinkEndpointId(l.target as string | ForceNode)}`,
      )
      .sort()
      .join(",");
    const fingerprint = `${nodeIds}|${linkIds}`;

    if (fingerprint === prevFingerprintRef.current) {
      // Structure unchanged — return the SAME object ref so ForceGraph2D doesn't restart
      return graphDataRef.current;
    }

    prevFingerprintRef.current = fingerprint;

    // Structure changed — build new graph data, applying cached positions
    const existingById = new Map<string, ForceNode>();
    for (const n of graphDataRef.current.nodes) {
      existingById.set(n.id, n);
    }

    const nodes = elements.nodes.map((node) => {
      // Check if this node already exists in the previous data (preserve simulation state)
      const existing = existingById.get(node.id);
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
          nodeType: node.nodeType,
          expandable: node.expandable,
          expanded: node.expanded,
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

      if (reheat) {
        simulationActiveRef.current = true;
        hullCacheRef.current = null;
        fg.d3ReheatSimulation();
      }
    },
    [settings.repulsion, settings.centerForce, settings.linkDistance, settings.nodeScale],
  );

  // Configure forces on mount or after render mode switch
  const needsZoomFitRef = useRef(true);

  // biome-ignore lint/correctness/useExhaustiveDependencies: renderMode triggers re-init
  useEffect(() => {
    if (forceInitializedRef.current) return;
    const timer = setTimeout(() => {
      configureForces(true);
      forceInitializedRef.current = true;
      needsZoomFitRef.current = true;
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

  // Keep a stable ref to the current paint context so canvas callbacks
  // can read it without triggering callback identity changes on every
  // hover / highlight state change. This avoids re-bindng the paint
  // callbacks to react-force-graph (which triggers a full redraw).
  const paintCtxRef = useRef<PaintContext>({
    nodeScale: settings.nodeScale,
    labelThreshold: settings.labelThreshold,
    activeNoteId: null,
    hoveredNodeId: null,
    neighborSet: new Set(),
    highlightedClusterId: null,
    highlightedNodeIds: null,
    linkWidth: settings.linkWidth,
    linkOpacity: settings.linkOpacity,
  });
  paintCtxRef.current = {
    nodeScale: settings.nodeScale,
    labelThreshold: settings.labelThreshold,
    activeNoteId,
    hoveredNodeId,
    neighborSet: neighborSetRef.current,
    highlightedClusterId,
    highlightedNodeIds: highlightedNodeIds.size > 0 ? highlightedNodeIds : null,
    linkWidth: settings.linkWidth,
    linkOpacity: settings.linkOpacity,
  };

  const revealedNodesRef = useRef(revealedNodes);
  revealedNodesRef.current = revealedNodes;

  const nodeCanvasObject = useCallback(
    (node: ForceNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const revealed = revealedNodesRef.current;
      if (revealed.size > 0 && !revealed.has(node.id)) return;
      paintNode(node, ctx, globalScale, paintCtxRef.current);
    },
    [],
  );

  const nodeCanvasObjectMode = useCallback(() => "replace" as const, []);

  const linkCanvasObject = useCallback(
    (link: ForceLink, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const revealed = revealedNodesRef.current;
      const source = link.source as unknown as ForceNode;
      const target = link.target as unknown as ForceNode;
      if (revealed.size > 0 && (!revealed.has(source.id) || !revealed.has(target.id))) {
        return;
      }
      paintLink(link, ctx, globalScale, paintCtxRef.current);
    },
    [],
  );

  const linkCanvasObjectMode = useCallback(() => "replace" as const, []);

  // Pointer area for hit detection
  const nodePointerAreaPaint = useCallback(
    (node: ForceNode, color: string, ctx: CanvasRenderingContext2D, _globalScale: number) => {
      const x = node.x ?? 0;
      const y = node.y ?? 0;
      ctx.fillStyle = color;

      if (node.nodeType === "entity") {
        const half =
          (Math.min(12 + Math.min(node.linkCount, 10) * 1.2, 24) / 2) * settings.nodeScale + 4;
        ctx.beginPath();
        ctx.moveTo(x, y - half);
        ctx.lineTo(x + half, y);
        ctx.lineTo(x, y + half);
        ctx.lineTo(x - half, y);
        ctx.closePath();
        ctx.fill();
      } else {
        const radius = (node.size / 2) * settings.nodeScale;
        ctx.beginPath();
        ctx.arc(x, y, radius + 4, 0, Math.PI * 2);
        ctx.fill();
      }
    },
    [settings.nodeScale],
  );

  // ── Interaction handlers ────────────────────────────────────────────

  // ── nodeType-aware click / double-click ───────────────────────────

  const handleSingleClick = useCallback(
    (node: ForceNode) => {
      switch (node.nodeType) {
        case "note": {
          onNodeClick(node.id);
          // If communities layer is on, glow same-community siblings
          if (node.clusterId.startsWith("community:")) {
            const siblings = new Set<string>();
            for (const n of elements.nodes) {
              if (n.clusterId === node.clusterId) siblings.add(n.id);
            }
            setHighlightedNodeIds(siblings);
          } else {
            setHighlightedNodeIds(new Set());
          }
          break;
        }
        case "entity": {
          // Highlight all connected notes (find entity edges in adjacency)
          const connected = adjacencyRef.current.get(node.id) ?? new Set<string>();
          const entityHighlight = new Set<string>(connected);
          entityHighlight.add(node.id);
          setHighlightedNodeIds(entityHighlight);
          break;
        }
        case "tree_section": {
          onNodeClick(node.id);
          // Highlight parent note + siblings
          const parentNoteId = node.id.split(":")[1]; // tree:noteId:treeNodeId
          const siblings = new Set<string>();
          siblings.add(node.id);
          if (parentNoteId) siblings.add(parentNoteId);
          for (const n of elements.nodes) {
            if (
              n.nodeType === "tree_section" &&
              n.id !== node.id &&
              n.id.startsWith(`tree:${parentNoteId}:`)
            ) {
              siblings.add(n.id);
            }
          }
          setHighlightedNodeIds(siblings);
          break;
        }
        case "tree_text":
          onNodeClick(node.id);
          break;
        default:
          onNodeClick(node.id);
          setHighlightedNodeIds(new Set());
          break;
      }
    },
    [onNodeClick, elements.nodes],
  );

  const handleDoubleClick = useCallback(
    (node: ForceNode) => {
      switch (node.nodeType) {
        case "note": {
          // Toggle tree expansion if tree layer is on
          if (node.expandable) {
            if (node.expanded) {
              onTreeCollapse?.(node.id);
            } else {
              onTreeExpand?.(node.id);
            }
          } else {
            onNodeDoubleClick?.(node.id);
          }
          break;
        }
        case "entity": {
          // Select entity — open in editor if handler exists, otherwise just select
          onNodeDoubleClick?.(node.id);
          break;
        }
        case "tree_section": {
          // Open the parent note in editor (at that heading)
          const parentNoteId = node.id.split(":")[1];
          if (parentNoteId) onNodeDoubleClick?.(parentNoteId);
          break;
        }
        default:
          onNodeDoubleClick?.(node.id);
          break;
      }
    },
    [onNodeDoubleClick, onTreeExpand, onTreeCollapse],
  );

  const handleNodeClick = useCallback(
    (node: ForceNode, _event: MouseEvent) => {
      const now = Date.now();
      if (
        lastClickRef.current &&
        lastClickRef.current.nodeId === node.id &&
        now - lastClickRef.current.time < 400
      ) {
        handleDoubleClick(node);
        lastClickRef.current = null;
        return;
      }
      lastClickRef.current = { nodeId: node.id, time: now };
      handleSingleClick(node);
    },
    [handleSingleClick, handleDoubleClick],
  );

  const handleNodeRightClick = useCallback(
    (node: ForceNode, event: MouseEvent) => {
      event.preventDefault();
      onNodeRightClick?.(node, { x: event.clientX, y: event.clientY });
    },
    [onNodeRightClick],
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

  // ── Drag-to-merge community tracking ───────────────────────────────

  const dragStartClusterRef = useRef<string | null>(null);

  const handleNodeDrag = useCallback((node: ForceNode) => {
    // Record the starting community on first drag movement
    if (dragStartClusterRef.current === null) {
      dragStartClusterRef.current = node.clusterId;
    }
    // Invalidate hull cache so it recomputes with the dragged position
    hullCacheRef.current = null;
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

      // Drag-to-merge: check if node was dropped in a different community's area
      const startCluster = dragStartClusterRef.current;
      dragStartClusterRef.current = null;

      if (
        onDragCommunityChange &&
        startCluster &&
        startCluster.startsWith("community:") &&
        node.nodeType === "note" &&
        node.x != null &&
        node.y != null
      ) {
        // Build convex hulls for each community
        const communityPoints = new Map<string, { x: number; y: number }[]>();
        for (const n of graphData.nodes) {
          if (!n.clusterId.startsWith("community:")) continue;
          if (n.x == null || n.y == null) continue;
          if (n.id === node.id) continue; // exclude dragged node
          if (!communityPoints.has(n.clusterId)) communityPoints.set(n.clusterId, []);
          communityPoints.get(n.clusterId)?.push({ x: n.x, y: n.y });
        }

        for (const [clusterId, points] of communityPoints) {
          if (clusterId === startCluster) continue;
          if (points.length < 3) continue;
          const hull = convexHull(points);
          if (hull.length < 3) continue;
          const expanded = expandHull(hull, 30);
          if (pointInPolygon(node.x, node.y, expanded)) {
            onDragCommunityChange({
              nodeId: node.id,
              nodeLabel: node.label,
              sourceCommunityId: startCluster,
              targetCommunityId: clusterId,
              targetCommunityName: clusterId.replace("community:", ""),
            });
            break;
          }
        }
      }

      // Save positions
      const positions = snapshotCurrentPositions();
      onSavePositions(positions);
    },
    [onNudge, onSavePositions, snapshotCurrentPositions, onDragCommunityChange, graphData.nodes],
  );

  const handleBackgroundClick = useCallback(() => {
    setHoveredNodeId(null);
    setHighlightedNodeIds(new Set());
    neighborSetRef.current = new Set();
  }, []);

  const handleEngineStop = useCallback(() => {
    simulationActiveRef.current = false;
    if (needsZoomFitRef.current) {
      needsZoomFitRef.current = false;
      graphRef.current?.zoomToFit(400, 60);
    }
    // Save positions when simulation settles
    onSavePositions(snapshotCurrentPositions());
  }, [onSavePositions, snapshotCurrentPositions]);

  // ── Community convex hull rendering (post-render) ────────────────────

  // Cache computed hulls so we skip convex hull math when nodes are stationary.
  // Invalidated by simulation ticks (onEngineStop) and node drags.
  const hullCacheRef = useRef<Map<
    string,
    { hull: { x: number; y: number }[]; color: string }
  > | null>(null);
  const simulationActiveRef = useRef(true);

  const onRenderFramePost = useCallback(
    (ctx: CanvasRenderingContext2D, _globalScale: number) => {
      // Rebuild hulls only when simulation is active (nodes moving) or cache is empty
      if (!hullCacheRef.current || simulationActiveRef.current) {
        const clusterPositions = new Map<string, { x: number; y: number; color: string }[]>();

        for (const node of graphData.nodes) {
          if (!node.clusterId.startsWith("community:")) continue;
          if (node.x == null || node.y == null) continue;

          if (!clusterPositions.has(node.clusterId)) {
            clusterPositions.set(node.clusterId, []);
          }
          clusterPositions.get(node.clusterId)?.push({
            x: node.x,
            y: node.y,
            color: node.color,
          });
        }

        const cache = new Map<string, { hull: { x: number; y: number }[]; color: string }>();
        for (const [clusterId, points] of clusterPositions) {
          if (points.length < 3) continue;
          const hull = convexHull(points);
          if (hull.length < 3) continue;
          cache.set(clusterId, {
            hull: expandHull(hull, 20),
            color: points[0].color,
          });
        }
        hullCacheRef.current = cache;
      }

      // Draw cached hulls
      for (const [, { hull: expanded, color }] of hullCacheRef.current) {
        ctx.save();
        ctx.setLineDash([4, 4]);
        ctx.strokeStyle = color.startsWith("#")
          ? `${color}26` // ~15% opacity hex
          : color;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(expanded[0].x, expanded[0].y);
        for (let i = 1; i < expanded.length; i++) {
          ctx.lineTo(expanded[i].x, expanded[i].y);
        }
        ctx.closePath();
        ctx.stroke();
        ctx.restore();
      }
    },
    [graphData.nodes],
  );

  // ── Zoom/pan controls ───────────────────────────────────────────────

  const zoomIn = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;
    fg.zoom(fg.zoom() * ZOOM_FACTOR, ZOOM_DURATION);
  }, []);

  const zoomOut = useCallback(() => {
    const fg = graphRef.current;
    if (!fg) return;
    fg.zoom(fg.zoom() / ZOOM_FACTOR, ZOOM_DURATION);
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
    onNodeRightClick: handleNodeRightClick,
    onNodeHover: handleNodeHover,
    onNodeDrag: handleNodeDrag,
    onNodeDragEnd: handleNodeDragEnd,
    onBackgroundClick: handleBackgroundClick,
    onEngineStop: handleEngineStop,
    onRenderFramePost,
    zoomIn,
    zoomOut,
    fitToScreen,
    getViewportBounds,
    snapshotPositions: snapshotCurrentPositions,
    configureForces,
    hoveredNodeId,
    highlightedNodeIds,
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
