import { useQuery } from "@shared/hooks/useQuery";
import type { Project } from "@shared/types";

export function useProject(id: string) {
  return useQuery<Project>("project_get", { id });
}
