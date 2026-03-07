import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationNodeDatum,
} from "d3-force";
import { Maximize2, Minus, Plus } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery } from "../../hooks/useQuery";
import type { Note, NoteLink } from "../../lib/types";

// ── Types ────────────────────────────────────────────────────────────────

interface GraphNode extends SimulationNodeDatum {
  id: string;
  title: string;
  linkCount: number;
}

interface GraphLink {
  source: string | GraphNode;
  target: string | GraphNode;
}

interface Transform {
  x: number;
  y: number;
  k: number; // scale
}

interface GraphViewProps {
  notes: Note[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
}

// ── Constants ────────────────────────────────────────────────────────────

const MIN_ZOOM = 0.15;
const MAX_ZOOM = 4;
const ZOOM_SENSITIVITY = 0.002;
const NODE_BASE_RADIUS = 4;
const NODE_SCALE_FACTOR = 1.5;
const LABEL_FONT_SIZE = 10;

// Node color palette — hardcoded rgba values required for SVG canvas rendering.
// CSS variables cannot be directly used in SVG attributes; extracting them via
// getComputedStyle would add significant complexity for minimal theming benefit.
const NODE_COLORS = [
  "rgba(96, 165, 250, 0.85)", // blue
  "rgba(167, 139, 250, 0.85)", // purple
  "rgba(52, 211, 153, 0.7)", // emerald
  "rgba(251, 146, 60, 0.7)", // orange
  "rgba(248, 113, 113, 0.6)", // red
  "rgba(56, 189, 248, 0.75)", // sky
  "rgba(192, 132, 252, 0.7)", // violet
];

function getNodeColor(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = (hash * 31 + id.charCodeAt(i)) | 0;
  }
  return NODE_COLORS[Math.abs(hash) % NODE_COLORS.length];
}

function getNodeRadius(linkCount: number): number {
  return NODE_BASE_RADIUS + Math.min(linkCount, 8) * NODE_SCALE_FACTOR;
}

// ── Component ────────────────────────────────────────────────────────────

export function GraphView({ notes, activeNoteId, onSelectNote }: GraphViewProps) {
  const { data: links } = useQuery<NoteLink[]>("note_links_all", undefined, []);
  const svgRef = useRef<SVGSVGElement>(null);
  const simRef = useRef<Simulation<GraphNode, GraphLink> | null>(null);
  const nodesRef = useRef<GraphNode[]>([]);
  const linksRef = useRef<GraphLink[]>([]);
  const nodeMapRef = useRef<Map<string, GraphNode>>(new Map());
  const rafRef = useRef(0);
  const [, setRenderKey] = useState(0);

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

  // Build and run simulation when notes/links change
  useEffect(() => {
    // Count links per node inline to avoid extra memo + dep
    const linkCountMap = new Map<string, number>();
    for (const l of links) {
      linkCountMap.set(l.sourceId, (linkCountMap.get(l.sourceId) || 0) + 1);
      linkCountMap.set(l.targetId, (linkCountMap.get(l.targetId) || 0) + 1);
    }

    const nodeMap = new Map<string, GraphNode>();
    for (const note of notes) {
      nodeMap.set(note.id, {
        id: note.id,
        title: note.title,
        linkCount: linkCountMap.get(note.id) || 0,
      });
    }

    const graphLinks: GraphLink[] = links
      .filter((l) => nodeMap.has(l.sourceId) && nodeMap.has(l.targetId))
      .map((l) => ({ source: l.sourceId, target: l.targetId }));

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
  }, [notes, links]);

  // ── Zoom (wheel) ────────────────────────────────────────────────────────
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const svg = svgRef.current;
    if (!svg) return;

    const rect = svg.getBoundingClientRect();
    // Cursor position relative to SVG element
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;

    setTransform((prev) => {
      const delta = -e.deltaY * ZOOM_SENSITIVITY;
      const newK = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, prev.k * (1 + delta)));
      const ratio = newK / prev.k;
      // Zoom toward cursor: adjust translate so cursor stays over same point
      return {
        k: newK,
        x: cx - (cx - prev.x) * ratio,
        y: cy - (cy - prev.y) * ratio,
      };
    });
  }, []);

  // ── Pan (background drag) ──────────────────────────────────────────────
  const handleBgPointerDown = useCallback((e: React.PointerEvent) => {
    // Only start pan if clicking background (not a node)
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
    // Handle pan
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

    // Handle node drag
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

  const nodes = nodesRef.current;
  const graphLinks = linksRef.current;

  return (
    <div className="flex-1 flex items-center justify-center overflow-hidden relative">
      {notes.length === 0 ? (
        <div className="text-muted text-sm">No notes to graph</div>
      ) : (
        <>
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
                return (
                  <line
                    key={`${s.id}-${t.id}`}
                    x1={s.x}
                    y1={s.y}
                    x2={t.x}
                    y2={t.y}
                    stroke={isActiveEdge ? "var(--brand-glow)" : "rgba(255,255,255,0.06)"}
                    strokeWidth={isActiveEdge ? 1.5 : 0.75}
                    className="transition-colors duration-300"
                  />
                );
              })}

              {/* Nodes */}
              {nodes.map((node) => {
                if (node.x == null || node.y == null) return null;
                const isActive = node.id === activeNoteId;
                const r = getNodeRadius(node.linkCount);
                const color = isActive ? "var(--color-brand)" : getNodeColor(node.id);

                return (
                  <g
                    key={node.id}
                    data-graph-node
                    transform={`translate(${node.x},${node.y})`}
                    onPointerDown={(e) => handleNodePointerDown(e, node.id)}
                    onClick={() => onSelectNote(node.id)}
                    className="cursor-pointer"
                    style={{ transition: "opacity 0.2s" }}
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

          {/* Zoom controls */}
          <div className="absolute bottom-3 right-3 flex flex-col gap-1">
            <button
              type="button"
              onClick={zoomIn}
              className="w-7 h-7 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-secondary hover:text-primary flex items-center justify-center transition-colors"
              aria-label="Zoom in"
            >
              <Plus className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={zoomOut}
              className="w-7 h-7 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-secondary hover:text-primary flex items-center justify-center transition-colors"
              aria-label="Zoom out"
            >
              <Minus className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={resetView}
              className="w-7 h-7 rounded-lg bg-white/[0.06] hover:bg-white/[0.1] text-secondary hover:text-primary flex items-center justify-center transition-colors"
              aria-label="Reset view"
            >
              <Maximize2 className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* Zoom level indicator */}
          <div className="absolute bottom-3 left-3 text-[10px] text-dim font-mono tabular-nums">
            {Math.round(transform.k * 100)}%
          </div>
        </>
      )}
    </div>
  );
}
