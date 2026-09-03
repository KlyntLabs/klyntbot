import { useEffect, useMemo } from "react";
import "../tasks.css";
import { PortalContainerProvider } from "@shared/lib/portal-container";
import { CreateIssueModal } from "../components/CreateIssueModal";
import { TabBar } from "../components/TabBar";
import { TabContent } from "../components/TabContent";
import { TasksLayout } from "../components/TasksLayout";
import { StatusWorkflowProvider } from "../contexts/StatusWorkflowContext";
import { useTasks } from "../hooks/useTasks";
import { TasksProvider } from "../hooks/useTasksContext";
import { useTabStore } from "../store/tab-store";

export function TasksPage() {
  return (
    <StatusWorkflowProvider projectId={null}>
      <TasksPageInner />
    </StatusWorkflowProvider>
  );
}

function TasksPageInner() {
  const tasksData = useTasks();
  const initFromAreas = useTabStore((s) => s.initFromAreas);

  useEffect(() => {
    if (tasksData.areas.length > 0) {
      initFromAreas(tasksData.areas);
    }
  }, [tasksData.areas, initFromAreas]);

  const ctxValue = useMemo(() => ({ refetch: tasksData.refetch }), [tasksData.refetch]);

  return (
    <TasksProvider value={ctxValue}>
      <PortalContainerProvider className="tasks-scope">
        <div className="tasks-scope flex-1 h-full min-w-0">
          <TasksLayout>
            <TabBar areas={tasksData.areas} projects={tasksData.projects} />
            <TabContent tasksData={tasksData} />
          </TasksLayout>
          <CreateIssueModal onCreateTask={tasksData.createTask} areas={tasksData.areas} />
        </div>
      </PortalContainerProvider>
    </TasksProvider>
  );
}
