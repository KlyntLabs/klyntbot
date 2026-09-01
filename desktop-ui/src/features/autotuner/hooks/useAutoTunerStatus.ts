import { useQuery } from "@shared/hooks/useQuery";
import type { AutoTunerStatus } from "../types";

export function useAutoTunerStatus() {
  return useQuery<AutoTunerStatus>("autotuner_status", undefined);
}
