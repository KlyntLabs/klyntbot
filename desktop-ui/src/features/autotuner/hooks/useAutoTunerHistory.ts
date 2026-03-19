import { useQuery } from "@shared/hooks/useQuery";
import type { ExperimentSummary } from "../types";

export function useAutoTunerHistory(limit = 20) {
  return useQuery<ExperimentSummary[]>("autotuner_history", { limit }, []);
}
