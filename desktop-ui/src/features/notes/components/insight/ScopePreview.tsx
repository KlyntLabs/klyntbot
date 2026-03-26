import { useQuery } from "@shared/hooks/useQuery";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type SimulationNodeDatum,
} from "d3-force";
import { Link, NotebookText } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ScopeConfig } from "./InsightScopePopover";

// ── Types ────────────────────────────────────────────────────────────────

interface ScopePreviewNote {
  id: string;
  title: string;
  notebookId: string | null;
}

interface ScopePreviewLink {
  sourceId: string;
  targetId: string;
}

interface ScopePreviewResponse {
  notes: ScopePreviewNote[];
  links: ScopePreviewLink[];
}

interface GraphNode extends SimulationNodeDatum {
  id: string;
  title: string;
  isCurrent: boolean;
}

interface GraphLink {
  source: string | GraphNode;
  target: string | GraphNode;
}

interface ScopePreviewProps {
  noteId: string;
  noteTitle?: string;
  scopeConfig: ScopeConfig;
}

const SCOPE_LABELS: Record<string, { label: string; icon: typeof Link }> = {
  backlinks: { label: "Linked", icon: Link },
  notebook: { label: "Notebook", icon: NotebookText },
};

// ── Component ────────────────────────────────────────────────────────────

export function ScopePreview({
  noteId,
  noteTitle = "Current note",
  scopeConfig,
}: ScopePreviewProps) {
  const { data, loading } = useQuery<ScopePreviewResponse>(
    "note_insight_preview_scope",
    { params: { noteId, scopeType: scopeConfig.scopeType } },
    { notes: [], links: [] },
  );

  const notes = data?.notes ?? [];
  const links = data?.links ?? [];
  const scope = SCOPE_LABELS[scopeConfig.scopeType] ?? SCOPE_LABELS.backlinks;
  const Icon = scope.icon;

  // Build graph data
  const { graphNodes, graphLinks } = useMemo(() => {
    const gn: GraphNode[] = [{ id: noteId, title: noteTitle, isCurrent: true }];
    for (const n of notes) {
      gn.push({ id: n.id, title: n.title, isCurrent: false });
    }

    const nodeSet = new Set(gn.map((n) => n.id));
    const gl: GraphLink[] = links
      .filter((l) => nodeSet.has(l.sourceId) && nodeSet.has(l.targetId))
      .map((l) => ({ source: l.sourceId, target: l.targetId }));

    return { graphNodes: gn, graphLinks: gl };
  }, [noteId, noteTitle, notes, links]);

  return (
    <div className="border-b border-border">
      {/* Header */}
      <div className="flex items-center gap-1.5 px-3 pt-2 pb-1 text-2xs text-muted-foreground">
        <Icon size={10} className="shrink-0" />
        <span className="font-medium">{scope.label} scope</span>
        <span className="text-dim">
          {loading ? "..." : `${notes.length} note${notes.length !== 1 ? "s" : ""}`}
        </span>
      </div>

      {/* Graph */}
      {graphNodes.length > 1 ? (
        <ScopeGraph nodes={graphNodes} links={graphLinks} />
      ) : !loading && notes.length === 0 ? (
        <div className="px-3 pb-2 text-[9px] text-dim">No related notes found</div>
      ) : null}
    </div>
  );
}

// ── Force Graph ──────────────────────────────────────────────────────────

function ScopeGraph({ nodes, links }: { nodes: GraphNode[]; links: GraphLink[] }) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [, setTick] = useState(0);
  const nodesRef = useRef<GraphNode[]>([]);
  const linksRef = useRef<GraphLink[]>([]);

  useEffect(() => {
    if (nodes.length === 0) return;

    const simNodes = nodes.map((n) => ({ ...n }));
    const simLinks = links.map((l) => ({ ...l }));
    nodesRef.current = simNodes;
    linksRef.current = simLinks;

    const sim = forceSimulation<GraphNode>(simNodes)
      .force(
        "link",
        forceLink<GraphNode, GraphLink>(simLinks)
          .id((d) => d.id)
          .distance(45)
          .strength(0.5),
      )
      .force("charge", forceManyBody().strength(-100).distanceMax(180))
      .force("center", forceCenter(0, 0).strength(0.15))
      .force(
        "collide",
        forceCollide<GraphNode>((d) => (d.isCurrent ? 12 : 8)),
      )
      .alphaDecay(0.06)
      .on("tick", () => setTick((k) => k + 1));

    return () => {
      sim.stop();
    };
  }, [nodes, links]);

  const renderNodes = nodesRef.current;
  const renderLinks = linksRef.current;

  return (
    <div className="px-1 pb-2">
      <svg ref={svgRef} width="100%" height="120" viewBox="-120 -60 240 120" className="w-full">
        <title>Scope graph</title>

        {/* Edges */}
        {renderLinks.map((link) => {
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
              stroke="rgba(255,255,255,0.08)"
              strokeWidth={0.75}
            />
          );
        })}

        {/* Nodes */}
        {renderNodes.map((node) => {
          if (node.x == null || node.y == null) return null;
          const r = node.isCurrent ? 5 : 3;
          const label = node.title.length > 20 ? `${node.title.slice(0, 20)}...` : node.title;

          return (
            <g key={node.id} transform={`translate(${node.x},${node.y})`}>
              {/* Glow for current node */}
              {node.isCurrent && <circle r={12} fill="var(--color-brand)" opacity={0.1} />}
              <circle
                r={r}
                fill={node.isCurrent ? "var(--color-brand)" : "rgba(255,255,255,0.45)"}
                stroke={node.isCurrent ? "var(--color-brand)" : "none"}
                strokeWidth={node.isCurrent ? 1.5 : 0}
              />
              <text
                y={-(r + 3)}
                textAnchor="middle"
                fill={node.isCurrent ? "rgba(255,255,255,0.9)" : "rgba(255,255,255,0.45)"}
                fontSize={node.isCurrent ? 6.5 : 5}
                fontWeight={node.isCurrent ? 600 : 400}
                className="select-none pointer-events-none"
              >
                {label}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}
