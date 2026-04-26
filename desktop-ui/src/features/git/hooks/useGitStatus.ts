import { useCallback } from "react";
import type { GitFileStatus, WorkspaceInfo } from "@/types";
import { getGitStatus } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

interface GitStatusState {
	branchName: string;
	files: GitFileStatus[];
	stagedFiles: GitFileStatus[];
	unstagedFiles: GitFileStatus[];
	totalAdditions: number;
	totalDeletions: number;
	error: string | null;
}

const emptyStatus: GitStatusState = {
	branchName: "",
	files: [],
	stagedFiles: [],
	unstagedFiles: [],
	totalAdditions: 0,
	totalDeletions: 0,
	error: null,
};

export function useGitStatus(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitStatusState>({
		queryKey: qk.git.status(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return emptyStatus;
			const data = await getGitStatus(activeWorkspace.id);
			return { ...data, error: null };
		},
		fallback: emptyStatus,
		enabled: activeWorkspace !== null,
		staleTime: 5_000,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		status: query.data,
		refresh,
	};
}
