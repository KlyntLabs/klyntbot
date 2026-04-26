import { getGitHubIssues } from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { GitHubIssue, WorkspaceInfo } from "@/types";

export function useGitHubIssues(activeWorkspace: WorkspaceInfo | null) {
  const workspaceId = activeWorkspace?.id ?? "";

  const query = useTauriQuery<{ issues: GitHubIssue[]; total: number }>({
    queryKey: qk.github.issues(workspaceId),
    queryFn: async () => {
      if (!activeWorkspace) return { issues: [], total: 0 };
      return await getGitHubIssues(activeWorkspace.id);
    },
    fallback: { issues: [], total: 0 },
    enabled: activeWorkspace !== null,
  });

  const refresh = useCallback(async () => {
    await query.refetch();
  }, [query]);

  return {
    issues: query.data.issues,
    total: query.data.total,
    isLoading: query.isLoading,
    error: query.error == null ? null : String(query.error),
    refresh,
  };
}
