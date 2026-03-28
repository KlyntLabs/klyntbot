import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type {
  FabricCommunityDetail,
  FabricEntity,
  FabricEntityEdge,
  FabricGraphBase,
  FabricLayer,
  FabricTreeNode,
} from "@shared/types/fabric";
import { useCallback, useState } from "react";

// ── Internal response shapes ──────────────────────────────────────────────

interface FabricEntitiesResponse {
  entities: FabricEntity[];
  edges: FabricEntityEdge[];
}

interface FabricTreeNodesResponse {
  noteId: string;
  nodes: FabricTreeNode[];
}

type FabricExpandResponse =
  | { type: "entities"; data: FabricEntitiesResponse }
  | { type: "tree"; data: FabricTreeNodesResponse[] }
  | { type: "communityDetail"; data: FabricCommunityDetail[] };

// ── Result type ───────────────────────────────────────────────────────────

export interface UseFabricGraphResult {
  base: FabricGraphBase | undefined;
  loading: boolean;
  entities: FabricEntity[];
  entityEdges: FabricEntityEdge[];
  expandedTrees: Map<string, FabricTreeNode[]>;
  communityDetails: Map<string, FabricCommunityDetail>;
  expandLayer: (layer: FabricLayer, scopes?: string[]) => Promise<void>;
  collapseTree: (noteId: string) => void;
  performAction: (action: string, payload: unknown) => Promise<void>;
  refetch: () => void;
}

// ── Hook ──────────────────────────────────────────────────────────────────

export function useFabricGraph(enabled: boolean): UseFabricGraphResult {
  const {
    data: base,
    loading,
    refetch,
  } = useQuery<FabricGraphBase | undefined>("fabric_graph_base", enabled ? undefined : null);

  const [entities, setEntities] = useState<FabricEntity[]>([]);
  const [entityEdges, setEntityEdges] = useState<FabricEntityEdge[]>([]);
  const [expandedTrees, setExpandedTrees] = useState<Map<string, FabricTreeNode[]>>(new Map());
  const [communityDetails, setCommunityDetails] = useState<Map<string, FabricCommunityDetail>>(
    new Map(),
  );

  const expandLayer = useCallback(async (layer: FabricLayer, scopes?: string[]) => {
    const response = await ipc<FabricExpandResponse>("fabric_graph_expand", {
      params: { layer, scopes: scopes ?? [] },
    });

    if (response.type === "entities") {
      setEntities(response.data.entities);
      setEntityEdges(response.data.edges);
    } else if (response.type === "tree") {
      setExpandedTrees((prev) => {
        const next = new Map(prev);
        for (const entry of response.data) {
          next.set(entry.noteId, entry.nodes);
        }
        return next;
      });
    } else if (response.type === "communityDetail") {
      setCommunityDetails((prev) => {
        const next = new Map(prev);
        for (const detail of response.data) {
          next.set(detail.communityId, detail);
        }
        return next;
      });
    }
  }, []);

  const collapseTree = useCallback((noteId: string) => {
    setExpandedTrees((prev) => {
      const next = new Map(prev);
      next.delete(noteId);
      return next;
    });
  }, []);

  const performAction = useCallback(async (action: string, payload: unknown) => {
    await ipc<{ success: boolean; message: string | null }>("fabric_graph_action", {
      params: { action, payload },
    });
  }, []);

  return {
    base,
    loading,
    entities,
    entityEdges,
    expandedTrees,
    communityDetails,
    expandLayer,
    collapseTree,
    performAction,
    refetch,
  };
}
