import { useQuery } from "@shared/hooks/useQuery";

export interface WeeklyStatPoint {
  date: string;
  reviews: number;
  atomsCreated: number;
}

export interface ReviewStatsSummary {
  streak: number;
  retention: number;
  weekly: WeeklyStatPoint[];
}

export function useReviewStats() {
  return useQuery<ReviewStatsSummary>(
    "review_stats_summary",
    undefined,
    {
      streak: 0,
      retention: 1.0,
      weekly: [],
    },
    { invalidateOn: ["entity:updated"] },
  );
}
