import { useQuery } from "@shared/hooks/useQuery";

export interface RetentionPoint {
  date: string;
  avgRetention: number;
  reviewCount: number;
}

export interface DomainHistory {
  domain: string;
  points: RetentionPoint[];
}

export interface RetentionHistoryData {
  overall: RetentionPoint[];
  domains: DomainHistory[];
}

export function useRetentionHistory(days: number = 30) {
  return useQuery<RetentionHistoryData>(
    "retention_history",
    { days, byDomain: true },
    {
      overall: [],
      domains: [],
    },
  );
}
