import { useMutation } from "@shared/hooks/useMutation";
import type { Project, ProjectUpdateParams } from "@shared/types";
import { useCallback } from "react";

const DEFAULT_ORDER = ["overview", "tasks", "okr", "notes"];

export function useProjectTabOrder(project: Project | undefined) {
  const tabOrder = (project?.settings as Record<string, unknown>)?.tabOrder as string[] | undefined;
  const order = tabOrder ?? DEFAULT_ORDER;

  const { mutate } = useMutation<Project, ProjectUpdateParams>("project_update", "params");

  const reorder = useCallback(
    async (newOrder: string[]) => {
      if (!project) return;
      const currentSettings = (project.settings ?? {}) as Record<string, unknown>;
      await mutate({ id: project.id, settings: { ...currentSettings, tabOrder: newOrder } });
    },
    [project, mutate],
  );

  return { order, reorder };
}
