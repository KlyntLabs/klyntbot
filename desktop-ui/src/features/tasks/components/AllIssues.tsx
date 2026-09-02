import { useCallback, useMemo } from "react";
import type { UseTasksResult } from "../hooks/useTasks";
import { filterIssues } from "../lib/mappers";
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
    async (issueId: string, status: string, statusLabelId: string | null) => {
      await updateTask.mutate({ id: issueId, status, statusLabelId });
    },
    [updateTask],
  );

  if (isSearchOpen) {
    if (searchQuery.trim()) {
      return <SearchIssues issues={issues} />;
    }
    return (
      <div className="px-6 py-8 text-center text-sm text-fg-secondary">
        Search results will appear here
      </div>
    );
  }

  return <IssueBoard issues={displayIssues} onUpdateStatus={handleUpdateStatus} />;
}
