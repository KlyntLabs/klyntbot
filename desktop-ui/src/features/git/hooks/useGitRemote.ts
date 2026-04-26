import { useCallback } from "react";
import type { WorkspaceInfo } from "@/types";
import { getGitRemote } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitRemote(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<string | null>({
		queryKey: qk.git.remote(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return null;
			return await getGitRemote(activeWorkspace.id);
		},
		fallback: null,
		enabled: activeWorkspace !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		remote: query.data,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
