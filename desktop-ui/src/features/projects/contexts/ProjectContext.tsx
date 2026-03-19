import { useEvent } from "@shared/hooks/useEvent";
import type { Objective, Project } from "@shared/types";
import { createContext, type ReactNode, useContext } from "react";
import { useProject } from "../hooks/useProject";
import { useProjectObjectives } from "../hooks/useProjectObjectives";

interface ProjectContextValue {
  project: Project | undefined;
  objectives: Objective[];
  loading: boolean;
  refetchProject: () => void;
  refetchObjectives: () => void;
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

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    const kind = payload?.entityKind;
    if (kind === "project") refetchProject();
    if (kind === "objective" || kind === "key_result") refetchObjectives();
  });

  return (
    <Ctx.Provider
      value={{
        project,
        objectives,
        loading: pLoading || oLoading,
        refetchProject,
        refetchObjectives,
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
