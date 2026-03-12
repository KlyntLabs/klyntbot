import { useMemo } from "react";
import { useIssuesStore } from "../store/issues-store";
import { IssueBoard } from "./IssueBoard";

interface ProjectViewProps {
  projectId: string;
}

export function ProjectView({ projectId }: ProjectViewProps) {
  const issues = useIssuesStore((s) => s.issues);

  const projectIssues = useMemo(
    () => issues.filter((issue) => issue.project?.id === projectId),
    [issues, projectId],
  );

  if (projectIssues.length === 0) {
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        No issues in this project
      </div>
    );
  }

  return <IssueBoard issues={projectIssues} />;
}
