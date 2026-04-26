import { useCallback } from "react";
import type { GitHubPullRequestDiff, WorkspaceInfo } from "@/types";
import { getGitHubPullRequestDiff } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitHubPullRequestDiffs(
	activeWorkspace: WorkspaceInfo | null,
	prNumber: number | null,
) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitHubPullRequestDiff[]>({
		queryKey: qk.github.diffsForPr(workspaceId, prNumber ?? -1),
		queryFn: async () => {
			if (!activeWorkspace || prNumber == null) return [];
			return await getGitHubPullRequestDiff(activeWorkspace.id, prNumber);
		},
		fallback: [],
		enabled: activeWorkspace !== null && prNumber !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		diffs: query.data,
		isLoading: query.isLoading,
		error: query.error == null
			? null
			: query.error instanceof Error
				? query.error.message
				: String(query.error),
		refresh,
	};
}
