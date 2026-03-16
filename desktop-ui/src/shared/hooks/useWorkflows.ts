import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { StatusLabel, StatusWorkflow } from "@shared/types";

/** Fetch all workflows (including templates) */
export function useWorkflows() {
  return useQuery<StatusWorkflow[]>("workflow_list", undefined, []);
}

/** Fetch effective labels for a project (or global default if null) */
export function useEffectiveLabels(projectId: string | null) {
  return useQuery<StatusLabel[]>("workflow_get_effective", { projectId }, []);
}

/** Mutations for workflow management */
export function useWorkflowMutations() {
  const create = useMutation<
    StatusWorkflow,
    {
      name: string;
      isTemplate?: boolean;
      sourceWorkflowId?: string;
    }
  >("workflow_create", "params");

  const deleteWf = useMutation<boolean, { id: string }>("workflow_delete");

  const createLabel = useMutation<
    StatusLabel,
    {
      workflowId: string;
      name: string;
      color: string;
      statusGroup: string;
      position?: number;
    }
  >("label_create", "params");

  const updateLabel = useMutation<
    StatusLabel,
    {
      id: string;
      name?: string;
      color?: string;
      statusGroup?: string;
      position?: number;
    }
  >("label_update", "params");

  const deleteLabel = useMutation<boolean, { id: string }>("label_delete");

  const reorderLabels = useMutation<
    void,
    {
      workflowId: string;
      labelIds: string[];
    }
  >("label_reorder", "params");

  return {
    create,
    delete: deleteWf,
    createLabel,
    updateLabel,
    deleteLabel,
    reorderLabels,
  };
}
