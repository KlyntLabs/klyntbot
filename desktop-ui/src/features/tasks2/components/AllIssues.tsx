import { useCallback, useMemo } from "react";
import type { UseTasksResult } from "../hooks/useTasks";
import { filterIssues } from "../lib/mappers";
import { status as allStatus } from "../lib/status-icons";
import { useFilterStore } from "../store/filter-store";
import { useSearchStore } from "../store/search-store";
import { IssueBoard } from "./IssueBoard";
import { SearchIssues } from "./SearchIssues";

interface AllIssuesProps {
  tasksData: UseTasksResult;
}

export default function AllIssues({ tasksData }: AllIssuesProps) {
  const { issues, updateTask } = tasksData;
  const { isSearchOpen, searchQuery } = useSearchStore();
  const filters = useFilterStore((s) => s.filters);
  const isFiltered = useFilterStore((s) => Object.values(s.filters).some((a) => a.length > 0));

  const displayIssues = useMemo(
    () => (isFiltered ? filterIssues(issues, filters) : issues),
    [isFiltered, filters, issues],
  );

  const handleUpdateStatus = useCallback(
    (issueId: string, statusId: string) => {
      // statusId is the UI status id (e.g. "in-progress"). Find the backend value.
      const statusDef = allStatus.find((s) => s.id === statusId);
      if (statusDef) {
        updateTask.mutate({ id: issueId, status: statusDef.backendStatus });
      }
    },
    [updateTask],
  );

  if (isSearchOpen) {
    if (searchQuery.trim()) {
      return <SearchIssues issues={issues} />;
    }
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        Search results will appear here
      </div>
    );
  }

  return <IssueBoard issues={displayIssues} onUpdateStatus={handleUpdateStatus} />;
}
