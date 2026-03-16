import { useEffect, useMemo } from "react";
import "../tasks2.css";
import { CreateIssueModal } from "../components/CreateIssueModal";
import { PortalContainerProvider } from "../components/portal-context";
import { TabBar } from "../components/TabBar";
import { TabContent } from "../components/TabContent";
import { Tasks2Layout } from "../components/Tasks2Layout";
import { StatusWorkflowProvider } from "../contexts/StatusWorkflowContext";
import { useTasks } from "../hooks/useTasks";
import { TasksProvider } from "../hooks/useTasksContext";
import { useTabStore } from "../store/tab-store";

export function Tasks2Page() {
  return (
    <StatusWorkflowProvider projectId={null}>
      <Tasks2PageInner />
    </StatusWorkflowProvider>
  );
}

function Tasks2PageInner() {
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
      <PortalContainerProvider>
        <div className="tasks2-scope flex-1 h-full min-w-0">
          <Tasks2Layout>
            <TabBar areas={tasksData.areas} projects={tasksData.projects} />
            <TabContent tasksData={tasksData} />
          </Tasks2Layout>
          <CreateIssueModal onCreateTask={tasksData.createTask} areas={tasksData.areas} />
        </div>
      </PortalContainerProvider>
    </TasksProvider>
  );
}
