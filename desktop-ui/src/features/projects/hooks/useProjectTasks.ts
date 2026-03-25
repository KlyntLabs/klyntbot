import { useQuery } from "@shared/hooks/useQuery";
import type { Task } from "@shared/types";

export function useProjectTasks(projectId: string) {
  return useQuery<Task[]>("task_list", { projectId }, []);
}
