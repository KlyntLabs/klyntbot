import { useMemo } from "react";
import { useFilterStore } from "../store/filter-store";
import { useIssuesStore } from "../store/issues-store";
import { useSearchStore } from "../store/search-store";
import { IssueBoard } from "./IssueBoard";
import { SearchIssues } from "./SearchIssues";

export default function AllIssues() {
  const issues = useIssuesStore((s) => s.issues);
  const filterIssues = useIssuesStore((s) => s.filterIssues);
  const { isSearchOpen, searchQuery } = useSearchStore();
  const filters = useFilterStore((s) => s.filters);
  const isFiltered = useFilterStore((s) => Object.values(s.filters).some((a) => a.length > 0));

  // All hooks must be called before any conditional returns
  const displayIssues = useMemo(
    () => (isFiltered ? filterIssues(filters) : issues),
    [isFiltered, filters, issues, filterIssues],
  );

  if (isSearchOpen) {
    if (searchQuery.trim()) {
      return <SearchIssues />;
    }
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        Search results will appear here
      </div>
    );
  }

  return <IssueBoard issues={displayIssues} />;
}
