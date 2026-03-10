import type { SessionSummary } from "../types/entity-links";
import { useQuery } from "./useQuery";

export function useProjectConversations(projectId: string | undefined) {
  return useQuery<SessionSummary[]>(
    "project_conversations_list",
    projectId ? { projectId } : null,
    [],
  );
}
