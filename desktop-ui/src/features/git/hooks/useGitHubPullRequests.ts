import { getGitHubPullRequests } from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { GitHubPullRequest, WorkspaceInfo } from "@/types";

export function useGitHubPullRequests(activeWorkspace: WorkspaceInfo | null) {
  const workspaceId = activeWorkspace?.id ?? "";

  const query = useTauriQuery<{
    pullRequests: GitHubPullRequest[];
    total: number;
  }>({
    queryKey: qk.github.pulls(workspaceId),
    staleTime: 60_000,
    queryFn: async () => {
      if (!activeWorkspace) return { pullRequests: [], total: 0 };
      return await getGitHubPullRequests(activeWorkspace.id);
    },
    fallback: { pullRequests: [], total: 0 },
    enabled: activeWorkspace !== null,
  });

  const refresh = useCallback(async () => {
    await query.refetch();
  }, [query]);

  return {
    pullRequests: query.data.pullRequests,
    total: query.data.total,
    isLoading: query.isLoading,
    error:
      query.error == null
        ? null
        : query.error instanceof Error
          ? query.error.message
          : String(query.error),
    refresh,
  };
}
