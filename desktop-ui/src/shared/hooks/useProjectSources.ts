import type { ProjectSource } from "../types/entity-links";
import { useQuery } from "./useQuery";

export function useProjectSources(projectId: string | undefined) {
  return useQuery<ProjectSource[]>("project_source_list", projectId ? { projectId } : null, []);
}
