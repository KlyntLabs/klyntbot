import type { Notebook } from "@shared/types";
import type {
  FabricCommunity,
  FabricEntity,
  FabricEntityEdge,
  FabricTreeNode,
} from "@shared/types/fabric";
import { useMemo } from "react";
import { computeFingerprint } from "../lib/graphFingerprint";
import { resolveLinkEndpointId } from "../lib/graphUtils";
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

export type ForceNodeType = "note" | "community_label" | "entity" | "tree_section" | "tree_text";

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
  nodeType: ForceNodeType;
  expandable?: boolean;
  expanded?: boolean;
  x?: number;
  y?: number;
  z?: number;
  vx?: number;
  vy?: number;
  fx?: number;
  fy?: number;
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

export interface FabricData {
  communities: FabricCommunity[];
  entities: FabricEntity[];
  entityEdges: FabricEntityEdge[];
  expandedTrees: Map<string, FabricTreeNode[]>;
  layerCommunities: boolean;
  layerEntities: boolean;
  layerTree: boolean;
}

interface UseGraphElementsParams {
  nodes: GraphNode[];
  links: GraphLink[];
  notebooks: Notebook[];
  clusteringMode: "notebook" | "semantic";
  fabricData?: FabricData;
}

export function useGraphElements({
  nodes,
  links,
  notebooks,
  clusteringMode,
  fabricData,
}: UseGraphElementsParams): GraphElements {
  return useMemo(() => {
    const clusterMap = new Map<string, ClusterInfo>();
    const notebookMap = new Map<string, Notebook>();
    for (const nb of notebooks) notebookMap.set(nb.id, nb);

    // Build community lookup when communities layer is active
    const communityByNoteId = new Map<string, FabricCommunity>();
    const useCommunities = clusteringMode === "semantic" && fabricData?.layerCommunities;
    if (useCommunities && fabricData?.communities) {
      for (const c of fabricData.communities) {
        for (const noteId of c.memberNoteIds) {
          communityByNoteId.set(noteId, c);
        }
      }
    }

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
      hasLinks.add(resolveLinkEndpointId(link.source));
      hasLinks.add(resolveLinkEndpointId(link.target));
    }

    // Use node.cluster from useGraphData when present (by-tag / by-notebook views),
    // otherwise fall back to notebook-based clustering
    const hasExplicitClusters = nodes.some((n) => n.cluster != null);

    for (const node of nodes) {
      if (useCommunities) {
        const community = communityByNoteId.get(node.id);
        if (community) {
          nodeClusterMap.set(node.id, `community:${community.id}`);
        } else if (hasLinks.has(node.id)) {
          nodeClusterMap.set(node.id, "_floating");
        } else {
          nodeClusterMap.set(node.id, "_isolated");
        }
      } else if (hasExplicitClusters && node.cluster != null) {
        nodeClusterMap.set(node.id, node.cluster);
      } else if (node.notebookId) {
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
      } else if (clusterId.startsWith("community:")) {
        const communityId = clusterId.replace("community:", "");
        const community = fabricData?.communities.find((c) => c.id === communityId);
        label = community?.name || "Unknown Community";
        color = community?.color || getClusterColor(clusterId);
      } else if (clusterId.startsWith("nb:")) {
        const nbId = clusterId.replace("nb:", "");
        const nb = notebookMap.get(nbId);
        label = nb?.title || "Unknown Notebook";
        color = getClusterColor(clusterId, nb);
      } else if (clusterId === "untagged" || clusterId === "no-notebook") {
        label = clusterId === "untagged" ? "Untagged" : "No Notebook";
        color = "#6B7280";
      } else {
        // Tag-based or other explicit clusters — use cluster string as label
        const nb = notebookMap.get(clusterId);
        label = nb?.title || clusterId;
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

      const isExpanded = fabricData?.expandedTrees.has(node.id) ?? false;

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
        nodeType: "note",
        expandable: fabricData?.layerTree ?? false,
        expanded: isExpanded,
      });
    }

    // ── Community label nodes ────────────────────────────────────────
    if (useCommunities && fabricData?.communities) {
      for (const community of fabricData.communities) {
        if (community.memberCount === 0) continue;
        const clusterId = `community:${community.id}`;
        const communityColor = clusterMap.get(clusterId)?.color || community.color || "#6B7280";
        forceNodes.push({
          id: `community_label:${community.id}`,
          label: `${community.name} (${community.memberCount})`,
          color: communityColor,
          size: 0, // size not used for community labels
          linkCount: community.memberCount,
          tags: [],
          bodyPreview: "",
          notebookId: null,
          clusterId,
          nodeType: "community_label",
        });
      }
    }

    // ── Entity nodes ──────────────────────────────────────────────────
    const entityLinks: ForceLink[] = [];
    if (fabricData?.layerEntities && fabricData.entities.length > 0) {
      const noteIdSet = new Set(forceNodes.map((n) => n.id));
      for (const entity of fabricData.entities) {
        forceNodes.push({
          id: `entity:${entity.id}`,
          label: entity.name,
          color: "#f59e0b",
          size: 12 + Math.min(entity.mentionCount, 10) * 2,
          linkCount: entity.mentionCount,
          tags: [entity.entityType],
          bodyPreview: `${entity.entityType} — ${entity.mentionCount} mentions`,
          notebookId: null,
          clusterId: "_entities",
          nodeType: "entity",
        });
      }
      // Add _entities cluster
      clusterMap.set("_entities", {
        id: "_entities",
        label: "Entities",
        color: "#f59e0b",
        count: fabricData.entities.length,
      });
      // Entity-to-note links
      for (const edge of fabricData.entityEdges) {
        if (noteIdSet.has(edge.noteId)) {
          entityLinks.push({
            source: `entity:${edge.entityId}`,
            target: edge.noteId,
            weight: edge.weight,
            color: "#f59e0b40",
          });
        }
      }
    }

    // ── Tree sub-nodes ───────────────────────────────────────────────
    const treeLinks: ForceLink[] = [];
    if (fabricData?.layerTree && fabricData.expandedTrees.size > 0) {
      for (const [noteId, treeNodes] of fabricData.expandedTrees) {
        const parentNode = forceNodes.find((n) => n.id === noteId);
        const parentColor = parentNode?.color || "#6B7280";
        for (const tn of treeNodes) {
          const treeNodeId = `tree:${noteId}:${tn.id}`;
          const treeNodeType: ForceNodeType =
            tn.nodeType === "section" ? "tree_section" : "tree_text";
          forceNodes.push({
            id: treeNodeId,
            label: tn.title || tn.contentPreview.slice(0, 40),
            color: `${parentColor}80`,
            size: treeNodeType === "tree_section" ? 14 : 10,
            linkCount: 0,
            tags: [],
            bodyPreview: tn.contentPreview,
            notebookId: null,
            clusterId: parentNode?.clusterId || "_floating",
            nodeType: treeNodeType,
          });
          // Link tree node to its parent (note or parent tree node)
          const linkTarget = tn.parentId == null ? noteId : `tree:${noteId}:${tn.parentId}`;
          treeLinks.push({
            source: treeNodeId,
            target: linkTarget,
            weight: 0.5,
            color: `${parentColor}40`,
          });
        }
      }
    }

    const edgeCounts = new Map<string, number>();
    for (const link of links) {
      const sourceId = resolveLinkEndpointId(link.source);
      const targetId = resolveLinkEndpointId(link.target);
      const key = [sourceId, targetId].sort().join(":");
      edgeCounts.set(key, (edgeCounts.get(key) || 0) + 1);
    }

    const forceLinks: ForceLink[] = [];
    const seenEdges = new Set<string>();
    for (const link of links) {
      const sourceId = resolveLinkEndpointId(link.source);
      const targetId = resolveLinkEndpointId(link.target);
      const key = [sourceId, targetId].sort().join(":");
      if (seenEdges.has(key)) continue;
      seenEdges.add(key);

      const count = edgeCounts.get(key) || 1;
      const weight = count === 1 ? 1 : count === 2 ? 1.8 : 2.8;
      const sourceCluster = nodeClusterMap.get(sourceId);
      const sourceColor = clusterMap.get(sourceCluster || "")?.color || "#6B7280";

      forceLinks.push({ source: sourceId, target: targetId, weight, color: sourceColor });
    }

    // Merge entity + tree links into the final link set
    const allForceLinks = [...forceLinks, ...entityLinks, ...treeLinks];

    const clusters = Array.from(clusterMap.values()).filter((c) => c.count > 0);

    const nodeIdList = forceNodes.map((n) => n.id);
    const edgePairList: [string, string][] = allForceLinks.map((l) => [l.source, l.target]);
    const fingerprint = computeFingerprint(nodeIdList, edgePairList);

    return { nodes: forceNodes, links: allForceLinks, clusters, fingerprint };
  }, [nodes, links, notebooks, clusteringMode, fabricData]);
}
