import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import type { Project } from "@shared/types";
import { useMemo, useState } from "react";
import { useParams } from "react-router";
import { MemorySection } from "../components/project-detail/entity-sections/MemorySection";
import { NoteSection } from "../components/project-detail/entity-sections/NoteSection";
import { OkrSection } from "../components/project-detail/entity-sections/OkrSection";
import { ProductivitySection } from "../components/project-detail/entity-sections/ProductivitySection";
import { SourceSection } from "../components/project-detail/entity-sections/SourceSection";
import { TaskSection } from "../components/project-detail/entity-sections/TaskSection";
import { ProjectChatInput } from "../components/project-detail/ProjectChatInput";
import { ProjectDetailHeader } from "../components/project-detail/ProjectDetailHeader";
import { ProjectEntityPanel } from "../components/project-detail/ProjectEntityPanel";
import { ProjectLeftPanel } from "../components/project-detail/ProjectLeftPanel";
import { ProjectTimeline } from "../components/project-detail/ProjectTimeline";
import { InstructionsPanel } from "../components/project-detail/panels/InstructionsPanel";
import { RolePanel } from "../components/project-detail/panels/RolePanel";
import { SourcesPanel } from "../components/project-detail/panels/SourcesPanel";

type PanelView = "none" | "instructions" | "sources" | "role";

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [activePanel, setActivePanel] = useState<PanelView>("none");

  const { data: allProjects, refetch: refetchProjects } = useQuery<Project[]>(
    "project_list",
    undefined,
    [],
  );

  useEvent<{ entityKind: string; id: string }>("entity:updated", (payload) => {
    const kind = payload?.entityKind;
    if (!kind || kind === "project") refetchProjects();
  });

  const project = useMemo(() => allProjects.find((p) => p.id === id), [id, allProjects]);

  if (!project) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-muted text-sm font-light">Project not found</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <ProjectDetailHeader project={project} />

      <div className="flex flex-1 overflow-hidden">
        {/* Left panel */}
        <ProjectLeftPanel
          project={project}
          onOpenInstructions={() =>
            setActivePanel((v) => (v === "instructions" ? "none" : "instructions"))
          }
          onOpenSources={() => setActivePanel((v) => (v === "sources" ? "none" : "sources"))}
          onOpenRole={() => setActivePanel((v) => (v === "role" ? "none" : "role"))}
        />

        {/* Slide panels */}
        {activePanel === "instructions" && (
          <InstructionsPanel project={project} onClose={() => setActivePanel("none")} />
        )}
        {activePanel === "sources" && (
          <SourcesPanel projectId={project.id} onClose={() => setActivePanel("none")} />
        )}
        {activePanel === "role" && (
          <RolePanel project={project} onClose={() => setActivePanel("none")} />
        )}

        {/* Center — Timeline */}
        <div className="flex-1 overflow-auto">
          <ProjectTimeline projectId={project.id} />
        </div>

        {/* Right — Entity sections */}
        <ProjectEntityPanel projectId={project.id}>
          <OkrSection projectId={project.id} defaultOpen />
          <TaskSection projectId={project.id} defaultOpen />
          <NoteSection projectId={project.id} defaultOpen />
          <MemorySection projectId={project.id} />
          <SourceSection projectId={project.id} />
          <ProductivitySection projectId={project.id} />
        </ProjectEntityPanel>
      </div>

      <ProjectChatInput projectId={project.id} />
    </div>
  );
}
