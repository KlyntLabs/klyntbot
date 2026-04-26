import type { LinkedEntities } from "../types/entity-links";
import { useQuery } from "./useQuery";

export function useEntityLinks(kind: string, id: string | undefined) {
  return useQuery<LinkedEntities>("entity_links_for_entity", id ? { kind, id } : null, {
    tasks: [],
    notes: [],
    conversations: [],
    sources: [],
    objectives: [],
    keyResults: [],
  });
}
