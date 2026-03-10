import type { TaskGroup } from "@shared/types";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";

/** Fetch groups for a project (or all if null) */
export function useGroups(projectId: string | null) {
  return useQuery<TaskGroup[]>("group_list", projectId !== null ? { projectId } : undefined, []);
}

/** Mutations for group management */
export function useGroupMutations() {
  const create = useMutation<
    TaskGroup,
    {
      projectId?: string;
      name: string;
      color?: string;
    }
  >("group_create", "params");

  const update = useMutation<
    TaskGroup,
    {
      id: string;
      name?: string;
      color?: string;
      position?: number;
    }
  >("group_update", "params");

  const deleteGroup = useMutation<boolean, { id: string }>("group_delete");

  const reorder = useMutation<
    void,
    {
      projectId?: string;
      groupIds: string[];
    }
  >("group_reorder", "params");

  return {
    create,
    update,
    delete: deleteGroup,
    reorder,
  };
}
