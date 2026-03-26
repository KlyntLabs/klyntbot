import { useQuery } from "@shared/hooks/useQuery";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type SimulationNodeDatum,
} from "d3-force";
import { Brain, FileText, Link, NotebookText, Radar, Zap } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ScopeConfig } from "./InsightScopePopover";

// ── Types ────────────────────────────────────────────────────────────────

interface ScopePreviewNote {
  id: string;
  title: string;
  notebookId: string | null;
  wordCount: number;
}

interface ScopePreviewLink {
  sourceId: string;
  targetId: string;
}

interface ContextSummary {
  totalNotes: number;
  totalWords: number;
  strongAtoms: number;
  fadingAtoms: number;
}

interface ScopePreviewResponse {
  notes: ScopePreviewNote[];
  links: ScopePreviewLink[];
  contextSummary: ContextSummary;
}

interface GraphNode extends SimulationNodeDatum {
  id: string;
  title: string;
  isCurrent: boolean;
  wordCount: number;
}

interface GraphLink {
  source: string | GraphNode;
  target: string | GraphNode;
  bidirectional: boolean;
}

interface ScopePreviewProps {
  noteId: string;
  noteTitle?: string;
  scopeConfig: ScopeConfig;
}

const SCOPE_LABELS: Record<string, { label: string; icon: typeof Link }> = {
  backlinks: { label: "Linked", icon: Link },
  semantic: { label: "Similar", icon: Radar },
  notebook: { label: "Notebook", icon: NotebookText },
};

const EMPTY: ScopePreviewResponse = {
  notes: [],
  links: [],
  contextSummary: { totalNotes: 0, totalWords: 0, strongAtoms: 0, fadingAtoms: 0 },
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
    EMPTY,
  );

  const notes = data?.notes ?? [];
  const links = data?.links ?? [];
  const summary = data?.contextSummary ?? EMPTY.contextSummary;
  const scope = SCOPE_LABELS[scopeConfig.scopeType] ?? SCOPE_LABELS.backlinks;
  const Icon = scope.icon;

  // Detect bidirectional links
  const linkPairSet = useMemo(() => {
    const set = new Set<string>();
    for (const l of links) set.add(`${l.sourceId}→${l.targetId}`);
    return set;
  }, [links]);

  // Build graph data
  const { graphNodes, graphLinks } = useMemo(() => {
    const gn: GraphNode[] = [{ id: noteId, title: noteTitle, isCurrent: true, wordCount: 0 }];
    for (const n of notes) {
      gn.push({ id: n.id, title: n.title, isCurrent: false, wordCount: n.wordCount });
    }

    const nodeSet = new Set(gn.map((n) => n.id));
    const seen = new Set<string>();
    const gl: GraphLink[] = [];

    for (const l of links) {
      if (!nodeSet.has(l.sourceId) || !nodeSet.has(l.targetId)) continue;
      const key = [l.sourceId, l.targetId].sort().join(":");
      if (seen.has(key)) continue;
      seen.add(key);

      const reverse = `${l.targetId}→${l.sourceId}`;
      gl.push({
        source: l.sourceId,
        target: l.targetId,
        bidirectional: linkPairSet.has(reverse),
      });
    }

    return { graphNodes: gn, graphLinks: gl };
  }, [noteId, noteTitle, notes, links, linkPairSet]);

  const hasContent = graphNodes.length > 1;

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
      {hasContent ? (
        <ScopeGraph nodes={graphNodes} links={graphLinks} />
      ) : !loading && notes.length === 0 ? (
        <div className="px-3 pb-1.5 text-[9px] text-dim">
          {scopeConfig.scopeType === "semantic"
            ? "No similar notes found — try Linked or Notebook scope"
            : "No related notes found"}
        </div>
      ) : null}

      {/* Context summary bar */}
      {!loading && (
        <div className="flex items-center gap-3 px-3 py-1.5 text-[9px] text-dim">
          <span className="flex items-center gap-1">
            <FileText size={8} />
            {summary.totalNotes} notes &middot; ~{summary.totalWords.toLocaleString()} words
          </span>
          {(summary.strongAtoms > 0 || summary.fadingAtoms > 0) && (
            <span className="flex items-center gap-1">
              <Brain size={8} />
              <span className="text-emerald-400">{summary.strongAtoms}</span>
              {summary.fadingAtoms > 0 && (
                <>
                  {" / "}
                  <span className="text-amber-400">{summary.fadingAtoms} fading</span>
                </>
              )}
            </span>
          )}
          {scopeConfig.includeCognitive && (
            <span className="flex items-center gap-1 text-purple-400">
              <Zap size={8} />
              cognitive
            </span>
          )}
        </div>
      )}
    </div>
  );
}

// ── Force Graph ──────────────────────────────────────────────────────────

function ScopeGraph({ nodes, links }: { nodes: GraphNode[]; links: GraphLink[] }) {
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
    <div className="px-1 pb-1">
      <svg width="100%" height="120" viewBox="-120 -60 240 120" className="w-full">
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
              stroke={link.bidirectional ? "rgba(167,139,250,0.25)" : "rgba(255,255,255,0.08)"}
              strokeWidth={link.bidirectional ? 1.2 : 0.6}
              strokeDasharray={link.bidirectional ? undefined : "3 2"}
            />
          );
        })}

        {/* Nodes */}
        {renderNodes.map((node) => {
          if (node.x == null || node.y == null) return null;
          // Scale node size by word count (min 3, max 6 for scope nodes)
          const baseR = node.isCurrent ? 6 : Math.min(6, Math.max(3, 3 + node.wordCount / 200));
          const label = node.title.length > 20 ? `${node.title.slice(0, 20)}...` : node.title;

          return (
            <g key={node.id} transform={`translate(${node.x},${node.y})`}>
              {/* Glow for current node */}
              {node.isCurrent && <circle r={14} fill="var(--color-brand)" opacity={0.1} />}
              <circle
                r={baseR}
                fill={node.isCurrent ? "var(--color-brand)" : "rgba(255,255,255,0.4)"}
                stroke={node.isCurrent ? "var(--color-brand)" : "none"}
                strokeWidth={node.isCurrent ? 1.5 : 0}
              />
              <text
                y={-(baseR + 3)}
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
