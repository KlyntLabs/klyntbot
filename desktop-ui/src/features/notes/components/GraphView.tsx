import { tagColor } from "@shared/lib/tagColor";
import type { Note } from "@shared/types";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
} from "d3-force";
import { Maximize2, Minus, Plus } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  type GraphLink,
  type GraphNode,
  type SmartView,
  useGraphData,
} from "../hooks/useGraphData";
import { GraphNodeTooltip } from "./GraphNodeTooltip";
import { GraphToolbar } from "./GraphToolbar";

// ── Types ────────────────────────────────────────────────────────────────

interface Transform {
  x: number;
  y: number;
  k: number; // scale
}

interface GraphViewProps {
  notes: Note[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  onOpenInEditor?: (id: string) => void;
}

// ── Constants ────────────────────────────────────────────────────────────

const MIN_ZOOM = 0.15;
const MAX_ZOOM = 4;
const ZOOM_SENSITIVITY = 0.002;
const NODE_BASE_RADIUS = 4;
const NODE_SCALE_FACTOR = 1.5;
const LABEL_FONT_SIZE = 10;

// Fallback color for notes without tags — hardcoded rgba required for SVG canvas rendering.
const DEFAULT_NODE_COLOR = "rgba(96, 165, 250, 0.85)";

function getNodeColor(node: GraphNode): string {
  if (node.primaryTag) {
    return tagColor(node.primaryTag);
  }
  return DEFAULT_NODE_COLOR;
}

function getNodeRadius(linkCount: number): number {
  return NODE_BASE_RADIUS + Math.min(linkCount, 8) * NODE_SCALE_FACTOR;
}

// ── Context Menu ─────────────────────────────────────────────────────────

interface ContextMenuState {
  nodeId: string;
  x: number;
  y: number;
}

function GraphContextMenu({
  state,
  onClose,
  onOpenInEditor,
  onShowNeighborhood,
  onDelete,
}: {
  state: ContextMenuState;
  onClose: () => void;
  onOpenInEditor: (id: string) => void;
  onShowNeighborhood: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  useEffect(() => {
    const handler = () => onClose();
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [onClose]);

  const items = [
    { label: "Open in editor", action: () => onOpenInEditor(state.nodeId) },
    { label: "Show neighborhood", action: () => onShowNeighborhood(state.nodeId) },
    { label: "Delete", action: () => onDelete(state.nodeId), danger: true },
  ];

  return createPortal(
    <div
      className="fixed z-[100] glass-panel rounded-xl py-1 min-w-[160px] shadow-lg"
      style={{ left: state.x, top: state.y }}
    >
      {items.map((item) => (
        <button
          key={item.label}
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            item.action();
            onClose();
          }}
          className={`w-full px-3 py-1.5 text-xs text-left transition-colors ${
            item.danger
              ? "text-destructive hover:bg-destructive/10"
              : "text-secondary hover:bg-surface-base"
          }`}
        >
          {item.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}

// ── Component ────────────────────────────────────────────────────────────

export function GraphView({ notes, activeNoteId, onSelectNote, onOpenInEditor }: GraphViewProps) {
  // Smart view state
  const [smartView, setSmartView] = useState<SmartView>("full");
  const [hopRadius, setHopRadius] = useState(2);
  const [searchQuery, setSearchQuery] = useState("");

  // Graph data from hook
  const { nodes: graphDataNodes, links: graphDataLinks } = useGraphData(
    smartView,
    notes,
    activeNoteId,
    hopRadius,
  );

  // Filter nodes by search query
  const filteredNodes = searchQuery
    ? graphDataNodes.filter((n) => n.title.toLowerCase().includes(searchQuery.toLowerCase()))
    : graphDataNodes;
  const filteredNodeIds = new Set(filteredNodes.map((n) => n.id));
  const filteredLinks = searchQuery
    ? graphDataLinks.filter((l) => {
        const sId = typeof l.source === "string" ? l.source : l.source.id;
        const tId = typeof l.target === "string" ? l.target : l.target.id;
        return filteredNodeIds.has(sId) && filteredNodeIds.has(tId);
      })
    : graphDataLinks;

  const svgRef = useRef<SVGSVGElement>(null);
  const simRef = useRef<Simulation<GraphNode, GraphLink> | null>(null);
  const nodesRef = useRef<GraphNode[]>([]);
  const linksRef = useRef<GraphLink[]>([]);
  const nodeMapRef = useRef<Map<string, GraphNode>>(new Map());
  const rafRef = useRef(0);
  const [, setRenderKey] = useState(0);

  // Hover state
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  // Context menu state
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);

  // Neighbor set for dimming
  const neighborSet = useRef<Set<string>>(new Set());

  // Transform state for zoom/pan — initialized to center once SVG mounts
  const [transform, setTransform] = useState<Transform>({ x: 0, y: 0, k: 1 });
  const transformRef = useRef(transform);
  transformRef.current = transform;
  const initializedRef = useRef(false);

  // Center the view once the SVG is mounted
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg || initializedRef.current) return;
    initializedRef.current = true;
    const rect = svg.getBoundingClientRect();
    setTransform({ x: rect.width / 2, y: rect.height / 2, k: 1 });
  }, []);

  // Pan state
  const panRef = useRef<{
    startX: number;
    startY: number;
    startTx: number;
    startTy: number;
  } | null>(null);

  // Drag state (node dragging)
  const dragRef = useRef<{
    nodeId: string;
    lastX: number;
    lastY: number;
  } | null>(null);

  // Build and run simulation when data changes
  useEffect(() => {
    const nodeMap = new Map<string, GraphNode>();
    // Clone nodes so d3 can mutate x/y
    for (const node of filteredNodes) {
      nodeMap.set(node.id, { ...node });
    }

    const graphLinks: GraphLink[] = filteredLinks
      .filter((l) => {
        const sId = typeof l.source === "string" ? l.source : l.source.id;
        const tId = typeof l.target === "string" ? l.target : l.target.id;
        return nodeMap.has(sId) && nodeMap.has(tId);
      })
      .map((l) => {
        const sId = typeof l.source === "string" ? l.source : l.source.id;
        const tId = typeof l.target === "string" ? l.target : l.target.id;
        return { source: sId, target: tId };
      });

    const graphNodes = Array.from(nodeMap.values());
    nodesRef.current = graphNodes;
    linksRef.current = graphLinks;
    nodeMapRef.current = nodeMap;

    let needsRender = false;
    const scheduleRender = () => {
      needsRender = true;
      if (!rafRef.current) {
        rafRef.current = requestAnimationFrame(() => {
          rafRef.current = 0;
          if (needsRender) {
            needsRender = false;
            setRenderKey((k) => k + 1);
          }
        });
      }
    };

    const sim = forceSimulation<GraphNode>(graphNodes)
      .force(
        "link",
        forceLink<GraphNode, GraphLink>(graphLinks)
          .id((d) => d.id)
          .distance(100)
          .strength(0.4),
      )
      .force("charge", forceManyBody().strength(-300).distanceMax(500))
      .force("center", forceCenter(0, 0).strength(0.05))
      .force(
        "collide",
        forceCollide<GraphNode>((d) => getNodeRadius(d.linkCount) + 8),
      )
      .alphaDecay(0.02)
      .on("tick", scheduleRender);

    simRef.current = sim;

    return () => {
      sim.stop();
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = 0;
      }
    };
  }, [filteredNodes, filteredLinks]);

  // Update neighbor set when hovered node changes
  useEffect(() => {
    const set = new Set<string>();
    if (hoveredNodeId) {
      set.add(hoveredNodeId);
      for (const link of linksRef.current) {
        const s = typeof link.source === "string" ? link.source : (link.source as GraphNode).id;
        const t = typeof link.target === "string" ? link.target : (link.target as GraphNode).id;
        if (s === hoveredNodeId) set.add(t);
        if (t === hoveredNodeId) set.add(s);
      }
    }
    neighborSet.current = set;
  }, [hoveredNodeId]);

  // ── Zoom (wheel) ────────────────────────────────────────────────────────
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const svg = svgRef.current;
    if (!svg) return;

    const rect = svg.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;

    setTransform((prev) => {
      const delta = -e.deltaY * ZOOM_SENSITIVITY;
      const newK = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, prev.k * (1 + delta)));
      const ratio = newK / prev.k;
      return {
        k: newK,
        x: cx - (cx - prev.x) * ratio,
        y: cy - (cy - prev.y) * ratio,
      };
    });
  }, []);

  // ── Pan (background drag) ──────────────────────────────────────────────
  const handleBgPointerDown = useCallback((e: React.PointerEvent) => {
    if ((e.target as Element).closest("[data-graph-node]")) return;
    e.preventDefault();
    (e.target as Element).setPointerCapture(e.pointerId);
    panRef.current = {
      startX: e.clientX,
      startY: e.clientY,
      startTx: transformRef.current.x,
      startTy: transformRef.current.y,
    };
  }, []);

  const handlePointerMove = useCallback((e: React.PointerEvent) => {
    if (panRef.current) {
      const pan = panRef.current;
      const dx = e.clientX - pan.startX;
      const dy = e.clientY - pan.startY;
      setTransform((prev) => ({
        ...prev,
        x: pan.startTx + dx,
        y: pan.startTy + dy,
      }));
      return;
    }

    if (!dragRef.current || !svgRef.current) return;
    const node = nodeMapRef.current.get(dragRef.current.nodeId);
    if (!node) return;
    const t = transformRef.current;
    const dx = (e.clientX - dragRef.current.lastX) / t.k;
    const dy = (e.clientY - dragRef.current.lastY) / t.k;
    node.fx = (node.fx ?? 0) + dx;
    node.fy = (node.fy ?? 0) + dy;
    dragRef.current.lastX = e.clientX;
    dragRef.current.lastY = e.clientY;
  }, []);

  const handlePointerUp = useCallback(() => {
    if (panRef.current) {
      panRef.current = null;
      return;
    }
    if (!dragRef.current) return;
    const node = nodeMapRef.current.get(dragRef.current.nodeId);
    if (node) {
      node.fx = null;
      node.fy = null;
    }
    dragRef.current = null;
    simRef.current?.alphaTarget(0);
  }, []);

  // Node drag
  const handleNodePointerDown = useCallback((e: React.PointerEvent, nodeId: string) => {
    e.preventDefault();
    e.stopPropagation();
    (e.target as Element).setPointerCapture(e.pointerId);
    const node = nodeMapRef.current.get(nodeId);
    if (node) {
      dragRef.current = { nodeId, lastX: e.clientX, lastY: e.clientY };
      node.fx = node.x;
      node.fy = node.y;
      simRef.current?.alphaTarget(0.3).restart();
    }
  }, []);

  // ── Zoom controls ──────────────────────────────────────────────────────
  const applyZoom = useCallback((factor: number) => {
    const svg = svgRef.current;
    if (!svg) return;
    const rect = svg.getBoundingClientRect();
    const cx = rect.width / 2;
    const cy = rect.height / 2;
    setTransform((prev) => {
      const newK = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, prev.k * factor));
      const ratio = newK / prev.k;
      return { k: newK, x: cx - (cx - prev.x) * ratio, y: cy - (cy - prev.y) * ratio };
    });
  }, []);

  const zoomIn = useCallback(() => applyZoom(1.3), [applyZoom]);
  const zoomOut = useCallback(() => applyZoom(1 / 1.3), [applyZoom]);

  const resetView = useCallback(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const rect = svg.getBoundingClientRect();
    setTransform({ x: rect.width / 2, y: rect.height / 2, k: 1 });
  }, []);

  // ── Event handlers ─────────────────────────────────────────────────────
  const handleNodeHover = useCallback((nodeId: string | null, e?: React.MouseEvent) => {
    setHoveredNodeId(nodeId);
    if (e) setTooltipPos({ x: e.clientX, y: e.clientY });
  }, []);

  const handleNodeClick = useCallback(
    (nodeId: string) => {
      onSelectNote(nodeId);
    },
    [onSelectNote],
  );

  const handleNodeDoubleClick = useCallback(
    (nodeId: string) => {
      onOpenInEditor?.(nodeId);
    },
    [onOpenInEditor],
  );

  const handleContextMenu = useCallback((e: React.MouseEvent, nodeId: string) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ nodeId, x: e.clientX, y: e.clientY });
  }, []);

  const handleShowNeighborhood = useCallback(
    (nodeId: string) => {
      onSelectNote(nodeId);
      setSmartView("local");
    },
    [onSelectNote],
  );

  const handleDeleteNode = useCallback(
    (nodeId: string) => {
      // Delete is handled by the parent — just select for now
      onSelectNote(nodeId);
    },
    [onSelectNote],
  );

  const nodes = nodesRef.current;
  const graphLinks = linksRef.current;
  const isHovering = hoveredNodeId !== null;
  const hoveredNode = hoveredNodeId ? nodeMapRef.current.get(hoveredNodeId) : null;

  return (
    <div className="flex-1 flex flex-col overflow-hidden relative">
      {/* Toolbar */}
      <GraphToolbar
        view={smartView}
        onViewChange={setSmartView}
        hopRadius={hopRadius}
        onHopRadiusChange={setHopRadius}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
      />

      {notes.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <div className="text-muted text-sm">No notes to graph</div>
        </div>
      ) : (
        <div className="flex-1 relative overflow-hidden">
          {/* SVG rendering — hardcoded rgba/hex color values below are required because
              CSS variables cannot be directly used in SVG attributes; extracting them via
              getComputedStyle would add significant complexity for minimal theming benefit. */}
          <svg
            ref={svgRef}
            className="w-full h-full"
            style={{ cursor: "grab" }}
            onWheel={handleWheel}
            onPointerDown={handleBgPointerDown}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
          >
            <title>Note link graph</title>
            <defs>
              {/* Active node glow gradient */}
              <radialGradient id="graph-active-glow">
                <stop offset="0%" stopColor="var(--color-brand)" stopOpacity="0.25" />
                <stop offset="60%" stopColor="var(--color-brand)" stopOpacity="0.08" />
                <stop offset="100%" stopColor="var(--color-brand)" stopOpacity="0" />
              </radialGradient>

              {/* Node hover glow */}
              <filter id="node-glow" x="-50%" y="-50%" width="200%" height="200%">
                <feGaussianBlur stdDeviation="3" result="blur" />
                <feMerge>
                  <feMergeNode in="blur" />
                  <feMergeNode in="SourceGraphic" />
                </feMerge>
              </filter>

              {/* Active node strong glow */}
              <filter id="active-glow" x="-100%" y="-100%" width="300%" height="300%">
                <feGaussianBlur stdDeviation="5" result="blur" />
                <feMerge>
                  <feMergeNode in="blur" />
                  <feMergeNode in="blur" />
                  <feMergeNode in="SourceGraphic" />
                </feMerge>
              </filter>

              {/* Background dot pattern */}
              <pattern
                id="graph-dots"
                x="0"
                y="0"
                width="24"
                height="24"
                patternUnits="userSpaceOnUse"
              >
                <circle cx="12" cy="12" r="0.5" fill="rgba(255,255,255,0.06)" />
              </pattern>
            </defs>

            {/* Background with dot pattern */}
            <rect width="100%" height="100%" fill="url(#graph-dots)" />

            {/* Transform group for zoom/pan */}
            <g transform={`translate(${transform.x},${transform.y}) scale(${transform.k})`}>
              {/* Edges */}
              {graphLinks.map((link) => {
                const s = link.source as GraphNode;
                const t = link.target as GraphNode;
                if (s.x == null || t.x == null) return null;
                const isActiveEdge = s.id === activeNoteId || t.id === activeNoteId;
                const isDimmed =
                  isHovering && !neighborSet.current.has(s.id) && !neighborSet.current.has(t.id);
                return (
                  <line
                    key={`${s.id}-${t.id}`}
                    x1={s.x}
                    y1={s.y}
                    x2={t.x}
                    y2={t.y}
                    stroke={isActiveEdge ? "var(--brand-glow)" : "rgba(255,255,255,0.06)"}
                    strokeWidth={isActiveEdge ? 1.5 : 0.75}
                    opacity={isDimmed ? 0.2 : 1}
                    className="transition-opacity duration-200"
                  />
                );
              })}

              {/* Nodes */}
              {nodes.map((node) => {
                if (node.x == null || node.y == null) return null;
                const isActive = node.id === activeNoteId;
                const r = getNodeRadius(node.linkCount);
                const color = isActive ? "var(--color-brand)" : getNodeColor(node);
                const isDimmed = isHovering && !neighborSet.current.has(node.id);

                return (
                  <g
                    key={node.id}
                    data-graph-node
                    transform={`translate(${node.x},${node.y})`}
                    onPointerDown={(e) => handleNodePointerDown(e, node.id)}
                    onClick={() => handleNodeClick(node.id)}
                    onDoubleClick={() => handleNodeDoubleClick(node.id)}
                    onContextMenu={(e) => handleContextMenu(e, node.id)}
                    onMouseEnter={(e) => handleNodeHover(node.id, e)}
                    onMouseMove={(e) => setTooltipPos({ x: e.clientX, y: e.clientY })}
                    onMouseLeave={() => handleNodeHover(null)}
                    className="cursor-pointer"
                    style={{ transition: "opacity 0.2s" }}
                    opacity={isDimmed ? 0.2 : 1}
                  >
                    {/* Active glow ring */}
                    {isActive && <circle r={r * 4} fill="url(#graph-active-glow)" />}

                    {/* Node circle */}
                    <circle
                      r={isActive ? r + 2 : r}
                      fill={color}
                      stroke={isActive ? "var(--color-brand)" : "rgba(255,255,255,0.08)"}
                      strokeWidth={isActive ? 2 : 0.5}
                      filter={isActive ? "url(#active-glow)" : undefined}
                      opacity={isActive ? 1 : 0.85}
                    />

                    {/* Inner highlight for depth */}
                    <circle
                      r={Math.max(1, (isActive ? r + 2 : r) * 0.4)}
                      fill="rgba(255,255,255,0.25)"
                      style={{ pointerEvents: "none" }}
                    />

                    {/* Label */}
                    <text
                      y={-(isActive ? r + 6 : r + 5)}
                      textAnchor="middle"
                      fill={isActive ? "#fff" : "rgba(255,255,255,0.55)"}
                      fontSize={isActive ? LABEL_FONT_SIZE + 1 : LABEL_FONT_SIZE}
                      fontWeight={isActive ? 600 : 400}
                      className="select-none pointer-events-none"
                      style={{ textShadow: "0 1px 3px rgba(0,0,0,0.8)" }}
                    >
                      {node.title.length > 24 ? `${node.title.slice(0, 24)}…` : node.title}
                    </text>
                  </g>
                );
              })}
            </g>
          </svg>

          {/* Tooltip */}
          {hoveredNode && <GraphNodeTooltip node={hoveredNode} x={tooltipPos.x} y={tooltipPos.y} />}

          {/* Context menu */}
          {contextMenu && (
            <GraphContextMenu
              state={contextMenu}
              onClose={() => setContextMenu(null)}
              onOpenInEditor={(id) => onOpenInEditor?.(id)}
              onShowNeighborhood={handleShowNeighborhood}
              onDelete={handleDeleteNode}
            />
          )}

          {/* Zoom controls */}
          <div className="absolute bottom-3 right-3 flex flex-col gap-1">
            <button
              type="button"
              onClick={zoomIn}
              className="w-7 h-7 rounded-lg bg-surface-base hover:bg-surface-raised text-secondary hover:text-primary flex items-center justify-center transition-colors"
              aria-label="Zoom in"
            >
              <Plus className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={zoomOut}
              className="w-7 h-7 rounded-lg bg-surface-base hover:bg-surface-raised text-secondary hover:text-primary flex items-center justify-center transition-colors"
              aria-label="Zoom out"
            >
              <Minus className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={resetView}
              className="w-7 h-7 rounded-lg bg-surface-base hover:bg-surface-raised text-secondary hover:text-primary flex items-center justify-center transition-colors"
              aria-label="Reset view"
            >
              <Maximize2 className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* Zoom level indicator */}
          <div className="absolute bottom-3 left-3 text-[10px] text-dim font-mono tabular-nums">
            {Math.round(transform.k * 100)}%
          </div>
        </div>
      )}
    </div>
  );
}
