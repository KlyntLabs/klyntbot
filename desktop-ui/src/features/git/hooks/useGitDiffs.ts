import { useCallback } from "react";
import type { WorkspaceInfo } from "@/types";
import { getGitDiffs } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

interface GitDiffState {
	diffs: Awaited<ReturnType<typeof getGitDiffs>>;
	isLoading: boolean;
	error: string | null;
}

export function useGitDiffs(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitDiffState["diffs"]>({
		queryKey: qk.git.diffs(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return [];
			return await getGitDiffs(activeWorkspace.id);
		},
		fallback: [],
		enabled: activeWorkspace !== null,
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
