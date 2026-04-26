import type { WorkspaceInfo } from "@/types";
import type { GitDiffSource, GitPanelMode } from "../types";
import { useGitHubIssues } from "../hooks/useGitHubIssues";
import { useGitHubPullRequests } from "../hooks/useGitHubPullRequests";
import { useGitHubPullRequestDiffs } from "../hooks/useGitHubPullRequestDiffs";
import { useGitHubPullRequestComments } from "../hooks/useGitHubPullRequestComments";

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
	useGitHubPullRequestDiffs(
		activeWorkspace,
		selectedPullRequestNumber ?? null,
	);
	useGitHubPullRequestComments(
		activeWorkspace,
		selectedPullRequestNumber ?? null,
	);

	return null;
}
