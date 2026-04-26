import { useCallback, useState } from "react";
import type { WorkspaceInfo } from "@/types";
import { listGitRoots } from "@services/tauri";
import { qk, useTauriMutation } from "@/lib/query";
import { useQueryClient } from "@tanstack/react-query";

export function useGitRepoScan(activeWorkspace: WorkspaceInfo | null) {
	const queryClient = useQueryClient();
	const [depth, setDepthState] = useState(2);

	const scan = useTauriMutation<string[], void>({
		mutationFn: async () => {
			if (!activeWorkspace) return [];
			return await listGitRoots(activeWorkspace.id, depth);
		},
		invalidates: [],
	});

	const repos =
		(activeWorkspace
			? queryClient.getQueryData<string[]>(
					qk.git.repoScan(activeWorkspace.id, depth),
				)
			: []) ?? [];

	const runScan = useCallback(async () => {
		const result = await scan.mutate();
		if (activeWorkspace) {
			queryClient.setQueryData(
				qk.git.repoScan(activeWorkspace.id, depth),
				result,
			);
		}
	}, [scan, activeWorkspace, queryClient, depth]);

	const clear = useCallback(() => {
		if (!activeWorkspace) return;
		queryClient.removeQueries({
			queryKey: qk.git.repoScan(activeWorkspace.id, depth),
		});
	}, [queryClient, activeWorkspace, depth]);

	return {
		repos,
		isLoading: scan.isLoading,
		error: scan.error == null ? null : String(scan.error),
		depth,
		hasScanned: repos.length > 0,
		scan: runScan,
		setDepth: setDepthState,
		clear,
	};
}
