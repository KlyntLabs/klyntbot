import { useMemo } from "react";
import { useIssuesStore } from "../store/issues-store";
import { useSearchStore } from "../store/search-store";
import { IssueLine } from "./IssueLine";

export function SearchIssues() {
  const issues = useIssuesStore((s) => s.issues);
  const searchQuery = useSearchStore((s) => s.searchQuery);

  const searchResults = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return [];
    return issues.filter(
      (issue) =>
        issue.title.toLowerCase().includes(q) ||
        issue.description.toLowerCase().includes(q) ||
        issue.identifier.toLowerCase().includes(q),
    );
  }, [issues, searchQuery]);

  return (
    <div className="w-full">
      {searchQuery.trim() !== "" && (
        <div>
          {searchResults.length > 0 ? (
            <div className="border border-[hsl(var(--border))] rounded-md mt-4">
              <div className="py-2 px-4 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))]/50">
                <h3 className="text-sm font-medium">Results ({searchResults.length})</h3>
              </div>
              <div className="divide-y divide-[hsl(var(--border))]">
                {searchResults.map((issue) => (
                  <IssueLine key={issue.id} issue={issue} />
                ))}
              </div>
            </div>
          ) : (
            <div className="text-center py-8 text-[hsl(var(--muted-foreground))]">
              No results found for &quot;{searchQuery}&quot;
            </div>
          )}
        </div>
      )}
    </div>
  );
}
