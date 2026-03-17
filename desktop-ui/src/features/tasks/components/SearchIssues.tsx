import { useMemo } from "react";
import type { Issue } from "../lib/mappers";
import { searchIssues } from "../lib/mappers";
import { useSearchStore } from "../store/search-store";
import { IssueLine } from "./IssueLine";

interface SearchIssuesProps {
  issues: Issue[];
}

export function SearchIssues({ issues }: SearchIssuesProps) {
  const searchQuery = useSearchStore((s) => s.searchQuery);

  const searchResults = useMemo(() => searchIssues(issues, searchQuery), [issues, searchQuery]);

  return (
    <div className="w-full">
      {searchQuery.trim() !== "" && (
        <div>
          {searchResults.length > 0 ? (
            <div className="border border-border rounded-md mt-4">
              <div className="py-2 px-4 border-b border-border bg-surface-low/50">
                <h3 className="text-sm font-medium">Results ({searchResults.length})</h3>
              </div>
              <div className="divide-y divide-border">
                {searchResults.map((issue) => (
                  <IssueLine key={issue.id} issue={issue} />
                ))}
              </div>
            </div>
          ) : (
            <div className="text-center py-8 text-muted">
              No results found for &quot;{searchQuery}&quot;
            </div>
          )}
        </div>
      )}
    </div>
  );
}
