import type { Objective, Project, Task } from "@shared/types";
import { createContext, type ReactNode, useContext, useEffect, useRef } from "react";
import { useProject } from "../hooks/useProject";
import { useProjectObjectives } from "../hooks/useProjectObjectives";
import { useProjectTasks } from "../hooks/useProjectTasks";
import { useProjectDetailStore } from "../store/project-detail-store";

interface ProjectContextValue {
  project: Project | undefined;
  objectives: Objective[];
  tasks: Task[];
  loading: boolean;
  refetchProject: () => void;
  refetchObjectives: () => void;
  refetchTasks: () => void;
}

const Ctx = createContext<ProjectContextValue | null>(null);

export function ProjectProvider({
  projectId,
  children,
}: {
  projectId: string;
  children: ReactNode;
}) {
  const { data: project, loading: pLoading, refetch: refetchProject } = useProject(projectId);
  const {
    data: objectives,
    loading: oLoading,
    refetch: refetchObjectives,
  } = useProjectObjectives(projectId);
  const { data: tasks, loading: tLoading, refetch: refetchTasks } = useProjectTasks(projectId);

  const prevProjectId = useRef(projectId);
  useEffect(() => {
    if (prevProjectId.current !== projectId) {
      prevProjectId.current = projectId;
      useProjectDetailStore.getState().reset();
    }
  });

  return (
    <Ctx.Provider
      value={{
        project,
        objectives,
        tasks,
        loading: pLoading || oLoading || tLoading,
        refetchProject,
        refetchObjectives,
        refetchTasks,
      }}
    >
      {children}
    </Ctx.Provider>
  );
}

export function useProjectContext() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useProjectContext must be used within ProjectProvider");
  return ctx;
}
