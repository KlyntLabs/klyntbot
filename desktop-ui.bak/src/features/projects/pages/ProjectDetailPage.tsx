// desktop-ui/src/features/projects/pages/ProjectDetailPage.tsx

import { Skeleton } from "@shared/ui";
import { lazy, Suspense, useMemo, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router";
import { GlassTabBar, type TabDef } from "../components/GlassTabBar";
import { ObjectiveCreateModal } from "../components/okr/ObjectiveCreateModal";
import { ProjectHeader } from "../components/ProjectHeader";
import { QuickAddFAB } from "../components/QuickAddFAB";
import { ProjectProvider, useProjectContext } from "../contexts/ProjectContext";
import { useProjectTabOrder } from "../hooks/useProjectTabOrder";

const OverviewTab = lazy(() =>
  import("../components/overview/OverviewTab").then((m) => ({ default: m.OverviewTab })),
);
const OkrTab = lazy(() => import("../components/okr/OkrTab").then((m) => ({ default: m.OkrTab })));
const ProjectTasksTab = lazy(() =>
  import("../components/tasks/ProjectTasksTab").then((m) => ({ default: m.ProjectTasksTab })),
);
const ProjectNotesTab = lazy(() =>
  import("../components/notes/ProjectNotesTab").then((m) => ({ default: m.ProjectNotesTab })),
);

const TAB_COMPONENTS: Record<string, React.LazyExoticComponent<React.ComponentType>> = {
  overview: OverviewTab,
  tasks: ProjectTasksTab,
  okr: OkrTab,
  notes: ProjectNotesTab,
};

function ProjectDetailInner() {
  const location = useLocation();
  const navigate = useNavigate();
  const { project, objectives } = useProjectContext();
  const [showCreateObjective, setShowCreateObjective] = useState(false);
  const { order, reorder } = useProjectTabOrder(project);

  // Derive active tab from URL path
  const pathParts = location.pathname.split("/");
  const lastSegment = pathParts[pathParts.length - 1];
  const activeTab = TAB_COMPONENTS[lastSegment] ? lastSegment : "overview";

  const basePath = `/project/${project?.id ?? ""}`;

  const tabs: TabDef[] = useMemo(() => {
    const taskCount = project ? Math.max(0, project.taskCount - project.completedCount) : 0;
    const okrProgress =
      objectives.length > 0
        ? Math.round(objectives.reduce((s, o) => s + o.progress, 0) / objectives.length)
        : 0;
    const defs: Record<string, TabDef> = {
      overview: { id: "overview", label: "Overview" },
      tasks: { id: "tasks", label: "Tasks", badge: taskCount > 0 ? taskCount : undefined },
      okr: { id: "okr", label: "OKR", badge: `${okrProgress}%` },
      notes: { id: "notes", label: "Notes" },
    };
    return order.map((id) => defs[id]).filter(Boolean);
  }, [order, project, objectives]);

  const ActiveComponent = TAB_COMPONENTS[activeTab] ?? OverviewTab;

  return (
    <div className="flex flex-col h-full">
      <ProjectHeader />
      <GlassTabBar tabs={tabs} activeTab={activeTab} basePath={basePath} onReorder={reorder} />
      <div className="flex-1 overflow-y-auto">
        <Suspense
          fallback={
            <div className="p-6">
              <Skeleton className="h-48 w-full" />
            </div>
          }
        >
          <ActiveComponent />
        </Suspense>
      </div>
      <QuickAddFAB
        onAddTask={() => {
          navigate(`/project/${project?.id}/tasks`);
        }}
        onAddNote={() => {
          navigate(`/project/${project?.id}/notes`);
        }}
        onAddObjective={() => setShowCreateObjective(true)}
      />
      {showCreateObjective && (
        <ObjectiveCreateModal
          open={showCreateObjective}
          onClose={() => setShowCreateObjective(false)}
        />
      )}
    </div>
  );
}

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  if (!id) return null;

  return (
    <ProjectProvider projectId={id}>
      <ProjectDetailInner />
    </ProjectProvider>
  );
}
