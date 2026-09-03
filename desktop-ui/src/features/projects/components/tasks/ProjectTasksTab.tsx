import { PortalContainerProvider } from "@shared/lib/portal-container";
import { useCallback, useMemo } from "react";
import { IssueBoard } from "../../../tasks/components/IssueBoard";
import { StatusWorkflowProvider } from "../../../tasks/contexts/StatusWorkflowContext";
import { useTasks } from "../../../tasks/hooks/useTasks";
import { TasksProvider } from "../../../tasks/hooks/useTasksContext";
import { useProjectContext } from "../../contexts/ProjectContext";

export function ProjectTasksTab() {
  const { project } = useProjectContext();

  if (!project) return null;

  return (
    <StatusWorkflowProvider projectId={project.id}>
      <ProjectTasksInner projectId={project.id} />
    </StatusWorkflowProvider>
  );
}

function ProjectTasksInner({ projectId }: { projectId: string }) {
  const tasksResult = useTasks();

  const ctxValue = useMemo(() => ({ refetch: tasksResult.refetch }), [tasksResult.refetch]);

  // Filter to project tasks only
  const projectIssues = useMemo(
    () => tasksResult.issues.filter((i) => i.project?.id === projectId),
    [tasksResult.issues, projectId],
  );

  const handleUpdateStatus = useCallback(
    (issueId: string, _status: string, statusLabelId: string | null) => {
      tasksResult.updateTask.mutate({
        id: issueId,
        statusLabelId,
      });
    },
    [tasksResult.updateTask],
  );

  return (
    <TasksProvider value={ctxValue}>
      <PortalContainerProvider className="tasks-scope">
        <div className="h-full">
          {projectIssues.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-center">
              <p className="text-sm text-fg-secondary mb-3">No tasks in this project yet.</p>
              <p className="text-ui-sm text-fg-secondary">
                Use the + button below to create your first task.
              </p>
            </div>
          ) : (
            <IssueBoard issues={projectIssues} onUpdateStatus={handleUpdateStatus} />
          )}
        </div>
      </PortalContainerProvider>
    </TasksProvider>
  );
}
