import { useQuery } from "@shared/hooks/useQuery";
import type { Objective } from "@shared/types";

export function useProjectObjectives(projectId: string) {
  return useQuery<Objective[]>("objective_list", { projectId }, []);
}
