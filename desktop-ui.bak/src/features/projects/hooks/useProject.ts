import { useQuery } from "@shared/hooks/useQuery";
import type { Project } from "@shared/types";

export function useProject(id: string) {
  return useQuery<Project>("project_get", { id }, undefined, {
    invalidateOn: ["entity:updated"],
    invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "project",
  });
}
