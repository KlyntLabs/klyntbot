import { useCallback } from "react";
import type { GitCommitDiff, WorkspaceInfo } from "@/types";
import { getGitCommitDiff } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitCommitDiffs(
	activeWorkspace: WorkspaceInfo | null,
	sha: string | null,
) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitCommitDiff[]>({
		queryKey: qk.git.commitDiffs(workspaceId, sha ?? ""),
		queryFn: async () => {
			if (!activeWorkspace || !sha) return [];
			return await getGitCommitDiff(activeWorkspace.id, sha);
		},
		fallback: [],
		enabled: activeWorkspace !== null && sha !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		diffs: query.data,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
