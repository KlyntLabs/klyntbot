import { getGitHubPullRequestComments } from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { GitHubPullRequestComment, WorkspaceInfo } from "@/types";

export function useGitHubPullRequestComments(
  activeWorkspace: WorkspaceInfo | null,
  prNumber: number | null,
) {
  const workspaceId = activeWorkspace?.id ?? "";

  const query = useTauriQuery<GitHubPullRequestComment[]>({
    queryKey: qk.github.commentsForPr(workspaceId, prNumber ?? -1),
    queryFn: async () => {
      if (!activeWorkspace || prNumber == null) return [];
      return await getGitHubPullRequestComments(activeWorkspace.id, prNumber);
    },
    fallback: [],
    enabled: activeWorkspace !== null && prNumber !== null,
  });

  const refresh = useCallback(async () => {
    await query.refetch();
  }, [query]);

  return {
    comments: query.data,
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
