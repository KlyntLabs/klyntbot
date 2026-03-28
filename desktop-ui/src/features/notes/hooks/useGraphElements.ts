import type { Notebook } from "@shared/types";
import { useMemo } from "react";
import { computeFingerprint } from "../lib/graphFingerprint";
import type { GraphLink, GraphNode } from "./useGraphData";

const CLUSTER_PALETTE = [
  "#a78bfa",
  "#93c5fd",
  "#6ee7b7",
  "#fcd34d",
  "#fca5a5",
  "#f9a8d4",
  "#a5b4fc",
  "#67e8f9",
  "#fdba74",
  "#86efac",
  "#c4b5fd",
  "#fde68a",
];

export interface ClusterInfo {
  id: string;
  label: string;
  color: string;
  count: number;
}

export interface ForceNode {
  id: string;
  label: string;
  color: string;
  size: number;
  linkCount: number;
  tags: string[];
  bodyPreview: string;
  notebookId: string | null;
  clusterId: string;
  x?: number;
  y?: number;
  z?: number;
  fx?: number | null;
  fy?: number | null;
}

export interface ForceLink {
  source: string;
  target: string;
  weight: number;
  color: string;
}

export interface GraphElements {
  nodes: ForceNode[];
  links: ForceLink[];
  clusters: ClusterInfo[];
  fingerprint: string;
}

function getNodeSize(linkCount: number): number {
  const normalized = Math.min(linkCount, 20) / 20;
  return 18 + normalized * 28;
}

interface UseGraphElementsParams {
  nodes: GraphNode[];
  links: GraphLink[];
  notebooks: Notebook[];
  clusteringMode: "notebook" | "semantic";
  activeNoteId: string | null;
}

export function useGraphElements({
  nodes,
  links,
  notebooks,
  clusteringMode,
  activeNoteId: _activeNoteId,
}: UseGraphElementsParams): GraphElements {
  return useMemo(() => {
    const clusterMap = new Map<string, ClusterInfo>();
    const notebookMap = new Map<string, Notebook>();
    for (const nb of notebooks) notebookMap.set(nb.id, nb);

    let colorIndex = 0;
    const getClusterColor = (id: string, notebook?: Notebook): string => {
      if (notebook?.color) return notebook.color;
      const existing = clusterMap.get(id);
      if (existing) return existing.color;
      return CLUSTER_PALETTE[colorIndex++ % CLUSTER_PALETTE.length];
    };

    const nodeClusterMap = new Map<string, string>();
    const hasLinks = new Set<string>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      hasLinks.add(sourceId);
      hasLinks.add(targetId);
    }
    for (const node of nodes) {
      if (node.notebookId) {
        nodeClusterMap.set(node.id, `nb:${node.notebookId}`);
      } else if (hasLinks.has(node.id)) {
        nodeClusterMap.set(node.id, "_floating");
      } else {
        nodeClusterMap.set(node.id, "_isolated");
      }
    }

    const seenClusters = new Set<string>();
    for (const [, clusterId] of nodeClusterMap) {
      if (seenClusters.has(clusterId)) continue;
      seenClusters.add(clusterId);

      let label: string;
      let color: string;

      if (clusterId === "_floating") {
        label = "Floating Ideas";
        color = "#9CA3AF";
      } else if (clusterId === "_isolated") {
        label = "Isolated Notes";
        color = "#6B7280";
      } else {
        const nbId = clusterId.replace("nb:", "");
        const nb = notebookMap.get(nbId);
        label = nb?.title || "Unknown Notebook";
        color = getClusterColor(clusterId, nb);
      }

      clusterMap.set(clusterId, { id: clusterId, label, color, count: 0 });
    }

    const forceNodes: ForceNode[] = [];
    for (const node of nodes) {
      const clusterId = nodeClusterMap.get(node.id) || "_isolated";
      const cluster = clusterMap.get(clusterId);
      if (cluster) cluster.count++;

      const color = cluster?.color || "#6B7280";
      const size = getNodeSize(node.linkCount);

      forceNodes.push({
        id: node.id,
        label: node.title,
        color,
        size,
        linkCount: node.linkCount,
        tags: node.tags,
        bodyPreview: node.bodyPreview,
        notebookId: node.notebookId,
        clusterId,
      });
    }

    const edgeCounts = new Map<string, number>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");
      edgeCounts.set(key, (edgeCounts.get(key) || 0) + 1);
    }

    const forceLinks: ForceLink[] = [];
    const seenEdges = new Set<string>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");
      if (seenEdges.has(key)) continue;
      seenEdges.add(key);

      const count = edgeCounts.get(key) || 1;
      const weight = count === 1 ? 1 : count === 2 ? 1.8 : 2.8;
      const sourceCluster = nodeClusterMap.get(sourceId);
      const sourceColor = clusterMap.get(sourceCluster || "")?.color || "#6B7280";

      forceLinks.push({ source: sourceId, target: targetId, weight, color: sourceColor });
    }

    const clusters = Array.from(clusterMap.values()).filter((c) => c.count > 0);

    const nodeIdList = nodes.map((n) => n.id);
    const edgePairList: [string, string][] = forceLinks.map((l) => [l.source, l.target]);
    const fingerprint = computeFingerprint(nodeIdList, edgePairList);

    return { nodes: forceNodes, links: forceLinks, clusters, fingerprint };
  }, [nodes, links, notebooks, clusteringMode]);
}
