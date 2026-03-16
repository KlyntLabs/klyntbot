import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import { tagColor } from "@shared/lib/tagColor";
import type { Note, NoteLink } from "@shared/types";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type SimulationNodeDatum,
} from "d3-force";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

// ── Types ────────────────────────────────────────────────────────────────

interface MiniNode extends SimulationNodeDatum {
  id: string;
  title: string;
  tags: string[];
  isCurrent: boolean;
}

interface MiniLink {
  source: string | MiniNode;
  target: string | MiniNode;
}

interface GraphMinimapProps {
  noteId: string | null;
  notes: Note[];
  onSelectNote: (id: string) => void;
  onExpandGraph: () => void;
}

// ── BFS to find nodes within N hops ──────────────────────────────────────

function getNeighborhood(centerId: string, links: NoteLink[], maxHops: number): Set<string> {
  const adjacency = new Map<string, string[]>();
  for (const l of links) {
    const s = adjacency.get(l.sourceId) ?? [];
    s.push(l.targetId);
    adjacency.set(l.sourceId, s);
    const t = adjacency.get(l.targetId) ?? [];
    t.push(l.sourceId);
    adjacency.set(l.targetId, t);
  }

  const visited = new Set<string>();
  const queue: Array<{ id: string; depth: number }> = [{ id: centerId, depth: 0 }];
  visited.add(centerId);

  while (queue.length > 0) {
    const current = queue.shift();
    if (!current) break;
    if (current.depth >= maxHops) continue;
    for (const neighbor of adjacency.get(current.id) ?? []) {
      if (!visited.has(neighbor)) {
        visited.add(neighbor);
        queue.push({ id: neighbor, depth: current.depth + 1 });
      }
    }
  }

  return visited;
}

// ── Component ────────────────────────────────────────────────────────────

export function GraphMinimap({ noteId, notes, onSelectNote, onExpandGraph }: GraphMinimapProps) {
  const { data: links, refetch: refetchLinks } = useQuery<NoteLink[]>(
    "note_links_all",
    undefined,
    [],
  );
  const [collapsed, setCollapsed] = useState(false);

  // Refetch graph links when any note is mutated
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload.entityKind === "note") refetchLinks();
  });
  const svgRef = useRef<SVGSVGElement>(null);
  const [, setTick] = useState(0);
  const nodesRef = useRef<MiniNode[]>([]);
  const linksRef = useRef<MiniLink[]>([]);

  // Build neighborhood subgraph
  const { subNodes, subLinks } = useMemo(() => {
    if (!noteId) return { subNodes: [], subLinks: [] };

    const neighborhood = getNeighborhood(noteId, links, 2);
    const noteMap = new Map<string, Note>();
    for (const n of notes) noteMap.set(n.id, n);

    const sn: MiniNode[] = [];
    for (const id of neighborhood) {
      const note = noteMap.get(id);
      if (note) {
        sn.push({
          id: note.id,
          title: note.title,
          tags: note.tags,
          isCurrent: id === noteId,
        });
      }
    }

    const nodeSet = new Set(sn.map((n) => n.id));
    const sl: MiniLink[] = links
      .filter((l) => nodeSet.has(l.sourceId) && nodeSet.has(l.targetId))
      .map((l) => ({ source: l.sourceId, target: l.targetId }));

    return { subNodes: sn, subLinks: sl };
  }, [noteId, notes, links]);

  // Run force simulation
  useEffect(() => {
    if (subNodes.length === 0) return;

    // Clone nodes to avoid mutating memo
    const simNodes = subNodes.map((n) => ({ ...n }));
    const simLinks = subLinks.map((l) => ({ ...l }));
    nodesRef.current = simNodes;
    linksRef.current = simLinks;

    const sim = forceSimulation<MiniNode>(simNodes)
      .force(
        "link",
        forceLink<MiniNode, MiniLink>(simLinks)
          .id((d) => d.id)
          .distance(40)
          .strength(0.6),
      )
      .force("charge", forceManyBody().strength(-120).distanceMax(200))
      .force("center", forceCenter(0, 0).strength(0.1))
      .force(
        "collide",
        forceCollide<MiniNode>((d) => (d.isCurrent ? 10 : 7)),
      )
      .alphaDecay(0.05)
      .on("tick", () => setTick((k) => k + 1));

    return () => {
      sim.stop();
    };
  }, [subNodes, subLinks]);

  const handleNodeClick = useCallback(
    (e: React.MouseEvent, id: string) => {
      e.stopPropagation();
      onSelectNote(id);
    },
    [onSelectNote],
  );

  if (!noteId) return null;

  const nodes = nodesRef.current;
  const graphLinks = linksRef.current;

  return (
    <div className="border-b border-border">
      <div className="flex items-center">
        <button
          type="button"
          onClick={() => setCollapsed(!collapsed)}
          className="flex-1 flex items-center gap-1.5 px-3 py-2 text-[10px] font-medium uppercase tracking-wider text-muted hover:text-secondary transition-colors"
        >
          {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
          <span>Local Graph</span>
        </button>
        <button
          type="button"
          onClick={onExpandGraph}
          className="text-[10px] text-dim hover:text-secondary px-2 py-1 transition-colors"
        >
          Expand
        </button>
      </div>

      {!collapsed && (
        <div className="px-1 pb-2">
          {nodes.length === 0 ? (
            <div className="text-[10px] text-dim px-2 py-3 text-center">No linked notes</div>
          ) : (
            <svg
              ref={svgRef}
              width="100%"
              height="140"
              viewBox="-100 -70 200 140"
              className="w-full"
            >
              <title>Local graph</title>
              {/* Edges */}
              {graphLinks.map((link) => {
                const s = link.source as MiniNode;
                const t = link.target as MiniNode;
                if (s.x == null || t.x == null) return null;
                return (
                  <line
                    key={`${s.id}-${t.id}`}
                    x1={s.x}
                    y1={s.y}
                    x2={t.x}
                    y2={t.y}
                    stroke="rgba(255,255,255,0.08)"
                    strokeWidth={0.75}
                  />
                );
              })}

              {/* Nodes */}
              {nodes.map((node) => {
                if (node.x == null || node.y == null) return null;
                const r = node.isCurrent ? 6 : 3.5;
                const color = node.isCurrent
                  ? "var(--color-brand)"
                  : node.tags.length > 0
                    ? tagColor(node.tags[0])
                    : "rgba(255,255,255,0.4)";

                return (
                  <g
                    key={node.id}
                    transform={`translate(${node.x},${node.y})`}
                    onClick={(e) => handleNodeClick(e, node.id)}
                    className="cursor-pointer"
                  >
                    {/* Glow for current node */}
                    {node.isCurrent && <circle r={14} fill="var(--color-brand)" opacity={0.12} />}
                    <circle
                      r={r}
                      fill={color}
                      stroke={node.isCurrent ? "var(--color-brand)" : "none"}
                      strokeWidth={node.isCurrent ? 1.5 : 0}
                      opacity={node.isCurrent ? 1 : 0.8}
                    />
                    {/* Label for current and nearby nodes */}
                    <text
                      y={-(r + 3)}
                      textAnchor="middle"
                      fill={node.isCurrent ? "rgba(255,255,255,0.9)" : "rgba(255,255,255,0.45)"}
                      fontSize={node.isCurrent ? 7 : 5.5}
                      fontWeight={node.isCurrent ? 600 : 400}
                      className="select-none pointer-events-none"
                    >
                      {node.title.length > 18 ? `${node.title.slice(0, 18)}...` : node.title}
                    </text>
                  </g>
                );
              })}
            </svg>
          )}
        </div>
      )}
    </div>
  );
}
