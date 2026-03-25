import { useQuery } from "@shared/hooks/useQuery";

export interface TopicHealth {
  id: string;
  name: string;
  domain: string;
  atomCount: number;
  avgRetention: number;
}

export interface KnowledgeHealthSummary {
  totalAtoms: number;
  activeAtoms: number;
  avgRetention: number;
  topics: TopicHealth[];
}

export function useKnowledgeHealth() {
  return useQuery<KnowledgeHealthSummary>("knowledge_health_summary", undefined, {
    totalAtoms: 0,
    activeAtoms: 0,
    avgRetention: 0,
    topics: [],
  });
}
