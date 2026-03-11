import {
  DataModeContext,
  LayerContext,
  SidebarContext,
  useLayerToggle,
} from "@features/dashboard/lib/layers";
import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import type { Project } from "@shared/types";
import { useMemo, useState } from "react";
import { useParams } from "react-router";
import { MemorySection } from "../components/project-detail/entity-sections/MemorySection";
import { NoteSection } from "../components/project-detail/entity-sections/NoteSection";
import { OkrSection } from "../components/project-detail/entity-sections/OkrSection";
import { ProductivitySection } from "../components/project-detail/entity-sections/ProductivitySection";
import { SourceSection } from "../components/project-detail/entity-sections/SourceSection";
import { TaskSection } from "../components/project-detail/entity-sections/TaskSection";
import { ProjectCalendarView } from "../components/project-detail/ProjectCalendarView";
import { ProjectChatInput } from "../components/project-detail/ProjectChatInput";
import { ProjectDetailHeader } from "../components/project-detail/ProjectDetailHeader";
import { ProjectEntityPanel } from "../components/project-detail/ProjectEntityPanel";
import { ProjectLeftPanel } from "../components/project-detail/ProjectLeftPanel";
import { InstructionsPanel } from "../components/project-detail/panels/InstructionsPanel";
import { RolePanel } from "../components/project-detail/panels/RolePanel";
import { SourcesPanel } from "../components/project-detail/panels/SourcesPanel";

type PanelView = "none" | "instructions" | "sources" | "role";

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [activePanel, setActivePanel] = useState<PanelView>("none");
  const [date, setDate] = useState(todayISO);

  // Layer state shared with dashboard; sidebar defaults closed on project page
  const { enabled, toggle, reset, enabledSources } = useLayerToggle();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const toggleSidebar = () => setSidebarOpen((prev) => !prev);

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
    <DataModeContext.Provider value="productivity">
      <LayerContext.Provider value={{ enabled, enabledSources }}>
        {/* Always false — DayColumnsView should never render its own SummaryPanel */}
        <SidebarContext.Provider value={false}>
          <div className="flex flex-col h-full gap-2 flex-1 min-w-0">
            <ProjectDetailHeader
              project={project}
              date={date}
              onDateChange={setDate}
              layersEnabled={enabled}
              onToggleLayer={toggle}
              onResetLayers={reset}
              sidebarOpen={sidebarOpen}
              onToggleSidebar={toggleSidebar}
            />

            <div className="flex flex-1 overflow-hidden gap-2">
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

              {/* Center — Dashboard calendar (no sidebar) */}
              <div className="flex-1 min-w-0 h-full overflow-hidden">
                <ProjectCalendarView date={date} projectId={project.id} />
              </div>

              {/* Right — Project entity sections */}
              {sidebarOpen && (
                <ProjectEntityPanel projectId={project.id}>
                  <OkrSection projectId={project.id} defaultOpen />
                  <TaskSection projectId={project.id} defaultOpen />
                  <NoteSection projectId={project.id} defaultOpen />
                  <MemorySection projectId={project.id} />
                  <SourceSection projectId={project.id} />
                  <ProductivitySection projectId={project.id} />
                </ProjectEntityPanel>
              )}
            </div>

            <ProjectChatInput projectId={project.id} />
          </div>
        </SidebarContext.Provider>
      </LayerContext.Provider>
    </DataModeContext.Provider>
  );
}
