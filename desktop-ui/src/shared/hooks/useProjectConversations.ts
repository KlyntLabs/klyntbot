import type { SessionSummary } from "../types/entity-links";
import { useQuery } from "./useQuery";

// TODO: Backend command "project_conversations_list" not yet implemented.
// This hook will return the empty default until the backend handler is added.
export function useProjectConversations(projectId: string | undefined) {
  return useQuery<SessionSummary[]>(
    "project_conversations_list",
    projectId ? { projectId } : null,
    [],
  );
}
