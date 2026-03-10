import { useQuery } from "./useQuery";

export interface SemanticFactSummary {
  id: string;
  domain: string;
  subject: string;
  predicate: string;
  object: string;
  confidence: number;
  source: string;
  validFrom: string;
  validUntil: string | null;
  stability: number;
  retrievability: number;
  lastAccessed: string | null;
  accessCount: number;
  status: string;
}

export function useProjectMemories(projectId: string | undefined, memoryType?: string) {
  const cmd = memoryType ? "project_memories_by_type" : "project_memories_list";
  const args = projectId ? (memoryType ? { projectId, memoryType } : { projectId }) : null;
  return useQuery<SemanticFactSummary[]>(cmd, args, []);
}
