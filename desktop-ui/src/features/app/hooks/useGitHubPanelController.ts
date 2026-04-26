import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { qk } from "@/lib/query";
import type {
  GitHubIssue,
  GitHubPullRequest,
  GitHubPullRequestComment,
  GitHubPullRequestDiff,
  WorkspaceInfo,
} from "@/types";

export function useGitHubPanelController(activeWorkspace: WorkspaceInfo | null) {
  const queryClient = useQueryClient();
  const workspaceId = activeWorkspace?.id ?? "";

  const [selectedPrNumber, setSelectedPrNumber] = useState<number | null>(null);

  const issues = queryClient.getQueryData<{
    issues: GitHubIssue[];
    total: number;
  }>(qk.github.issues(workspaceId)) ?? { issues: [], total: 0 };

  const pullRequests = queryClient.getQueryData<{
    pullRequests: GitHubPullRequest[];
    total: number;
  }>(qk.github.pulls(workspaceId)) ?? {
    pullRequests: [],
    total: 0,
  };

  const diffs = selectedPrNumber
    ? (queryClient.getQueryData<GitHubPullRequestDiff[]>(
        qk.github.diffsForPr(workspaceId, selectedPrNumber),
      ) ?? [])
    : [];

  const comments = selectedPrNumber
    ? (queryClient.getQueryData<GitHubPullRequestComment[]>(
        qk.github.commentsForPr(workspaceId, selectedPrNumber),
      ) ?? [])
    : [];

  const resetGitHubPanelState = useCallback(() => {
    setSelectedPrNumber(null);
  }, []);

  return {
    gitIssues: issues.issues,
    gitIssuesTotal: issues.total,
    gitIssuesLoading: false,
    gitIssuesError: null,
    gitPullRequests: pullRequests.pullRequests,
    gitPullRequestsTotal: pullRequests.total,
    gitPullRequestsLoading: false,
    gitPullRequestsError: null,
    gitPullRequestDiffs: diffs,
    gitPullRequestDiffsLoading: false,
    gitPullRequestDiffsError: null,
    gitPullRequestComments: comments,
    gitPullRequestCommentsLoading: false,
    gitPullRequestCommentsError: null,
    // Setters become noops — child hooks now write directly to the cache.
    // Kept for back-compat with caller signatures.
    handleGitIssuesChange: () => {},
    handleGitPullRequestsChange: () => {},
    handleGitPullRequestDiffsChange: () => {},
    handleGitPullRequestCommentsChange: () => {},
    resetGitHubPanelState,
    selectedPrNumber,
    setSelectedPrNumber,
  };
}
