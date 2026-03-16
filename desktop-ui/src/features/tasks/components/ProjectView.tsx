import { useCallback, useMemo } from "react";
import type { UseTasksResult } from "../hooks/useTasks";
import { IssueBoard } from "./IssueBoard";

interface ProjectViewProps {
  projectId: string;
  tasksData: UseTasksResult;
}

export function ProjectView({ projectId, tasksData }: ProjectViewProps) {
  const projectIssues = useMemo(
    () => tasksData.issues.filter((issue) => issue.project?.id === projectId),
    [tasksData.issues, projectId],
  );

  const handleUpdateStatus = useCallback(
    (issueId: string, statusId: string) => {
      tasksData.updateTask.mutate({ id: issueId, statusLabelId: statusId });
    },
    [tasksData.updateTask],
  );

  if (projectIssues.length === 0) {
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        No issues in this project
      </div>
    );
  }

  return <IssueBoard issues={projectIssues} onUpdateStatus={handleUpdateStatus} />;
}
