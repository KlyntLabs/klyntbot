import { useQuery } from "@shared/hooks/useQuery";
import type { NoteLink, NoteListItem } from "@shared/types";
import { useMemo } from "react";

// ── Types ────────────────────────────────────────────────────────────────

export type SmartView = "local" | "full" | "by-tag" | "by-notebook" | "orphans";

export interface GraphNode {
  id: string;
  title: string;
  linkCount: number;
  primaryTag: string | null;
  notebookId: string | null;
  bodyPreview: string;
  tags: string[];
  cluster?: string;
}

export interface GraphLink {
  source: string | GraphNode;
  target: string | GraphNode;
}

interface GraphData {
  nodes: GraphNode[];
  links: GraphLink[];
}

// ── BFS helper ───────────────────────────────────────────────────────────

function bfs(startId: string, adjacency: Map<string, Set<string>>, maxHops: number): Set<string> {
  const visited = new Set<string>();
  const queue: [string, number][] = [[startId, 0]];
  visited.add(startId);

  while (queue.length > 0) {
    const entry = queue.shift();
    if (!entry) break;
    const [current, depth] = entry;
    if (depth >= maxHops) continue;
    const neighbors = adjacency.get(current);
    if (!neighbors) continue;
    for (const neighbor of neighbors) {
      if (!visited.has(neighbor)) {
        visited.add(neighbor);
        queue.push([neighbor, depth + 1]);
      }
    }
  }

  return visited;
}

// ── Hook ─────────────────────────────────────────────────────────────────

export function useGraphData(
  view: SmartView,
  notes: NoteListItem[],
  noteId: string | null,
  hopRadius: number,
): GraphData {
  const { data: links } = useQuery<NoteLink[]>("note_links_all", undefined, [], {
    invalidateOn: ["entity:updated"],
    invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "note",
  });

  return useMemo(() => {
    // Build adjacency map and link count map
    const adjacency = new Map<string, Set<string>>();
    const linkCountMap = new Map<string, number>();

    for (const l of links) {
      linkCountMap.set(l.sourceId, (linkCountMap.get(l.sourceId) || 0) + 1);
      linkCountMap.set(l.targetId, (linkCountMap.get(l.targetId) || 0) + 1);

      if (!adjacency.has(l.sourceId)) adjacency.set(l.sourceId, new Set());
      if (!adjacency.has(l.targetId)) adjacency.set(l.targetId, new Set());
      adjacency.get(l.sourceId)?.add(l.targetId);
      adjacency.get(l.targetId)?.add(l.sourceId);
    }

    // Build full node map
    const allNodes = new Map<string, GraphNode>();
    for (const note of notes) {
      const bodyLines = ("" as string).split("\n").filter(Boolean);
      allNodes.set(note.id, {
        id: note.id,
        title: note.title,
        linkCount: linkCountMap.get(note.id) || 0,
        primaryTag: note.tags.length > 0 ? note.tags[0] : null,
        notebookId: note.notebookId,
        bodyPreview: bodyLines.slice(0, 2).join("\n").slice(0, 120),
        tags: note.tags,
      });
    }

    let includedIds: Set<string>;

    switch (view) {
      case "local": {
        if (!noteId || !allNodes.has(noteId)) {
          return { nodes: [], links: [] };
        }
        includedIds = bfs(noteId, adjacency, hopRadius);
        break;
      }
      case "orphans": {
        includedIds = new Set<string>();
        for (const [id] of allNodes) {
          if ((linkCountMap.get(id) || 0) === 0) {
            includedIds.add(id);
          }
        }
        break;
      }
      case "by-tag": {
        includedIds = new Set(allNodes.keys());
        // Add cluster metadata
        for (const [, node] of allNodes) {
          node.cluster = node.primaryTag || "untagged";
        }
        break;
      }
      case "by-notebook": {
        includedIds = new Set(allNodes.keys());
        for (const [, node] of allNodes) {
          node.cluster = node.notebookId || "no-notebook";
        }
        break;
      }
      default: {
        includedIds = new Set(allNodes.keys());
        break;
      }
    }

    const filteredNodes: GraphNode[] = [];
    for (const id of includedIds) {
      const node = allNodes.get(id);
      if (node) filteredNodes.push(node);
    }

    const filteredLinks: GraphLink[] = links
      .filter((l) => includedIds.has(l.sourceId) && includedIds.has(l.targetId))
      .map((l) => ({ source: l.sourceId, target: l.targetId }));

    return { nodes: filteredNodes, links: filteredLinks };
  }, [view, notes, noteId, hopRadius, links]);
}
