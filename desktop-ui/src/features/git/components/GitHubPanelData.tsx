import type { WorkspaceInfo } from "@/types";
import { useGitHubIssues } from "../hooks/useGitHubIssues";
import { useGitHubPullRequestComments } from "../hooks/useGitHubPullRequestComments";
import { useGitHubPullRequestDiffs } from "../hooks/useGitHubPullRequestDiffs";
import { useGitHubPullRequests } from "../hooks/useGitHubPullRequests";
import type { GitDiffSource, GitPanelMode } from "../types";

type GitHubPanelDataProps = {
  activeWorkspace: WorkspaceInfo | null;
  gitPanelMode: GitPanelMode;
  shouldLoadDiffs: boolean;
  diffSource: GitDiffSource;
  selectedPullRequestNumber: number | null;
};

export function GitHubPanelData({
  activeWorkspace,
  selectedPullRequestNumber,
}: GitHubPanelDataProps) {
  useGitHubIssues(activeWorkspace);
  useGitHubPullRequests(activeWorkspace);
  useGitHubPullRequestDiffs(activeWorkspace, selectedPullRequestNumber ?? null);
  useGitHubPullRequestComments(activeWorkspace, selectedPullRequestNumber ?? null);

  return null;
}
