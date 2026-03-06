import {
  forceCenter,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationNodeDatum,
} from "d3-force";
import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery } from "../../hooks/useQuery";
import type { Note, NoteLink } from "../../lib/types";

interface GraphNode extends SimulationNodeDatum {
  id: string;
  title: string;
}

interface GraphLink {
  source: string | GraphNode;
  target: string | GraphNode;
}

interface GraphViewProps {
  notes: Note[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
}

export function GraphView({ notes, activeNoteId, onSelectNote }: GraphViewProps) {
  const { data: links } = useQuery<NoteLink[]>("note_links_all", undefined, []);
  const svgRef = useRef<SVGSVGElement>(null);
  const simRef = useRef<Simulation<GraphNode, GraphLink> | null>(null);
  const nodesRef = useRef<GraphNode[]>([]);
  const linksRef = useRef<GraphLink[]>([]);
  const nodeMapRef = useRef<Map<string, GraphNode>>(new Map());
  const rafRef = useRef(0);
  const [, setRenderKey] = useState(0);

  // Dragging state
  const dragRef = useRef<{
    nodeId: string;
    lastX: number;
    lastY: number;
  } | null>(null);

  // Build and run simulation when notes/links change
  useEffect(() => {
    const nodeMap = new Map<string, GraphNode>();
    for (const note of notes) {
      nodeMap.set(note.id, { id: note.id, title: note.title });
    }

    const graphLinks: GraphLink[] = links
      .filter((l) => nodeMap.has(l.sourceId) && nodeMap.has(l.targetId))
      .map((l) => ({ source: l.sourceId, target: l.targetId }));

    const graphNodes = Array.from(nodeMap.values());
    nodesRef.current = graphNodes;
    linksRef.current = graphLinks;
    nodeMapRef.current = nodeMap;

    // Use rAF to batch simulation ticks into frames instead of setState per tick
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
          .distance(80),
      )
      .force("charge", forceManyBody().strength(-200))
      .force("center", forceCenter(0, 0))
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

  // Drag handlers — use nodeMapRef for O(1) lookup
  const handlePointerDown = useCallback((e: React.PointerEvent, nodeId: string) => {
    e.preventDefault();
    (e.target as Element).setPointerCapture(e.pointerId);
    const node = nodeMapRef.current.get(nodeId);
    if (node) {
      dragRef.current = { nodeId, lastX: e.clientX, lastY: e.clientY };
      node.fx = node.x;
      node.fy = node.y;
      simRef.current?.alphaTarget(0.3).restart();
    }
  }, []);

  const handlePointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragRef.current || !svgRef.current) return;
    const node = nodeMapRef.current.get(dragRef.current.nodeId);
    if (!node) return;
    const svg = svgRef.current;
    const rect = svg.getBoundingClientRect();
    const scaleX = svg.viewBox.baseVal.width / rect.width;
    const scaleY = svg.viewBox.baseVal.height / rect.height;
    node.fx = (node.fx ?? 0) + (e.clientX - dragRef.current.lastX) * scaleX;
    node.fy = (node.fy ?? 0) + (e.clientY - dragRef.current.lastY) * scaleY;
    dragRef.current.lastX = e.clientX;
    dragRef.current.lastY = e.clientY;
  }, []);

  const handlePointerUp = useCallback(() => {
    if (!dragRef.current) return;
    const node = nodeMapRef.current.get(dragRef.current.nodeId);
    if (node) {
      node.fx = null;
      node.fy = null;
    }
    dragRef.current = null;
    simRef.current?.alphaTarget(0);
  }, []);

  const nodes = nodesRef.current;
  const graphLinks = linksRef.current;

  return (
    <div className="flex-1 glass-panel rounded-2xl flex items-center justify-center overflow-hidden">
      {notes.length === 0 ? (
        <div className="text-muted text-sm">No notes to graph</div>
      ) : (
        <svg
          ref={svgRef}
          viewBox="-400 -300 800 600"
          className="w-full h-full"
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
        >
          <title>Note link graph</title>
          {/* Edges */}
          {graphLinks.map((link) => {
            const s = link.source as GraphNode;
            const t = link.target as GraphNode;
            if (s.x == null || t.x == null) return null;
            return (
              <line
                key={`${s.id}-${t.id}`}
                x1={s.x}
                y1={s.y}
                x2={t.x}
                y2={t.y}
                stroke="var(--color-border)"
                strokeWidth={1}
                strokeOpacity={0.6}
              />
            );
          })}

          {/* Nodes */}
          {nodes.map((node) => {
            if (node.x == null || node.y == null) return null;
            const isActive = node.id === activeNoteId;
            return (
              <g
                key={node.id}
                transform={`translate(${node.x},${node.y})`}
                onPointerDown={(e) => handlePointerDown(e, node.id)}
                onClick={() => onSelectNote(node.id)}
                className="cursor-pointer"
              >
                <circle
                  r={isActive ? 8 : 6}
                  fill={isActive ? "var(--color-brand)" : "var(--color-surface-raised)"}
                  stroke={isActive ? "var(--color-brand)" : "var(--color-border)"}
                  strokeWidth={1.5}
                />
                <text
                  y={-12}
                  textAnchor="middle"
                  fill="var(--color-text-secondary)"
                  fontSize={10}
                  className="select-none pointer-events-none"
                >
                  {node.title.length > 20 ? `${node.title.slice(0, 20)}...` : node.title}
                </text>
              </g>
            );
          })}
        </svg>
      )}
    </div>
  );
}
