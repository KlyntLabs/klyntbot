import { useQuery } from "@shared/hooks/useQuery";
import type { Objective } from "@shared/types";

export function useProjectObjectives(projectId: string) {
  return useQuery<Objective[]>("objective_list", { projectId }, [], {
    invalidateOn: ["entity:updated"],
    invalidateFilter: (p) => {
      const kind = (p as { entityKind?: string })?.entityKind;
      return kind === "objective" || kind === "key_result";
    },
  });
}
