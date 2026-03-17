import type { Notebook } from "@shared/types";
import type { ElementDefinition } from "cytoscape";
import { useMemo } from "react";
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

export type ClusterMode = "notebook" | "ai" | "hybrid";

export interface ClusterInfo {
  id: string;
  label: string;
  color: string;
  count: number;
}

function getNodeSize(linkCount: number): number {
  const normalized = Math.min(linkCount, 30) / 30;
  return 12 + normalized * 28;
}

interface UseCytoscapeElementsParams {
  nodes: GraphNode[];
  links: GraphLink[];
  notebooks: Notebook[];
  clusterMode: ClusterMode;
  activeNoteId: string | null;
}

export function useCytoscapeElements({
  nodes,
  links,
  notebooks,
  clusterMode,
  activeNoteId,
}: UseCytoscapeElementsParams): { elements: ElementDefinition[]; clusters: ClusterInfo[] } {
  return useMemo(() => {
    const elements: ElementDefinition[] = [];
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
    if (clusterMode === "notebook") {
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
    }

    const seenClusters = new Set<string>();
    for (const [, clusterId] of nodeClusterMap) {
      if (seenClusters.has(clusterId)) continue;
      seenClusters.add(clusterId);

      let label: string;
      let color: string;
      let type = "notebook";

      if (clusterId === "_floating") {
        label = "Floating Ideas";
        color = "#9CA3AF";
        type = "orphan-linked";
      } else if (clusterId === "_isolated") {
        label = "Isolated Notes";
        color = "#6B7280";
        type = "orphan-isolated";
      } else {
        const nbId = clusterId.replace("nb:", "");
        const nb = notebookMap.get(nbId);
        label = nb?.title || "Unknown Notebook";
        color = getClusterColor(clusterId, nb);
      }

      clusterMap.set(clusterId, { id: clusterId, label, color, count: 0 });
      elements.push({ group: "nodes", data: { id: clusterId, label, color, type } });
    }

    for (const node of nodes) {
      const clusterId = nodeClusterMap.get(node.id) || "_isolated";
      const cluster = clusterMap.get(clusterId);
      if (cluster) cluster.count++;

      const color = cluster?.color || "#6B7280";
      const size = getNodeSize(node.linkCount);

      elements.push({
        group: "nodes",
        data: {
          id: node.id,
          label: node.title,
          parent: clusterId,
          color,
          size,
          linkCount: node.linkCount,
          bodyPreview: node.bodyPreview,
          tags: node.tags,
          notebookId: node.notebookId,
        },
      });
    }

    const edgePairs = new Map<string, number>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");
      edgePairs.set(key, (edgePairs.get(key) || 0) + 1);
    }

    const seenEdges = new Set<string>();
    for (const link of links) {
      const sourceId = typeof link.source === "string" ? link.source : link.source.id;
      const targetId = typeof link.target === "string" ? link.target : link.target.id;
      const key = [sourceId, targetId].sort().join(":");
      if (seenEdges.has(key)) continue;
      seenEdges.add(key);

      const count = edgePairs.get(key) || 1;
      const weight = count === 1 ? 1 : count === 2 ? 1.8 : 2.8;
      const sourceCluster = nodeClusterMap.get(sourceId);
      const sourceColor = clusterMap.get(sourceCluster || "")?.color || "#6B7280";

      elements.push({
        group: "edges",
        data: { id: `e:${key}`, source: sourceId, target: targetId, weight, sourceColor },
      });
    }

    const clusters = Array.from(clusterMap.values()).filter((c) => c.count > 0);
    return { elements, clusters };
  }, [nodes, links, notebooks, clusterMode, activeNoteId]);
}
