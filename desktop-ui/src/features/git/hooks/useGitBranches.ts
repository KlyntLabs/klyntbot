import { useCallback, useMemo, useState } from "react";
import type { BranchInfo, WorkspaceInfo } from "@/types";
import {
	checkoutGitBranch,
	checkoutGitHubPullRequest,
	createGitBranch,
	listGitBranches,
} from "@services/tauri";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";

export function useGitBranches(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";
	const [error, setError] = useState<string | null>(null);

	const query = useTauriQuery<BranchInfo[]>({
		queryKey: qk.git.branches(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return [];
			return await listGitBranches(activeWorkspace.id);
		},
		fallback: [],
		enabled: activeWorkspace !== null,
	});

	const branches = useMemo(
		() =>
			[...query.data].sort((a, b) =>
				(b.lastCommit ?? 0) - (a.lastCommit ?? 0),
			),
		[query.data],
	);

	const checkout = useTauriMutation<void, { name: string }>({
		mutationFn: async ({ name }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await checkoutGitBranch(activeWorkspace.id, name);
		},
		invalidates: [qk.git.branches(workspaceId), qk.git.status(workspaceId)],
		onError: (e) => setError(String(e)),
	});

	const checkoutPr = useTauriMutation<void, { prNumber: number }>({
		mutationFn: async ({ prNumber }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await checkoutGitHubPullRequest(activeWorkspace.id, prNumber);
		},
		invalidates: [qk.git.branches(workspaceId), qk.git.status(workspaceId)],
		onError: (e) => setError(String(e)),
	});

	const createBranch = useTauriMutation<void, { name: string }>({
		mutationFn: async ({ name }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await createGitBranch(activeWorkspace.id, name);
		},
		invalidates: [qk.git.branches(workspaceId), qk.git.status(workspaceId)],
		onError: (e) => setError(String(e)),
	});

	const refreshBranches = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		branches,
		error,
		refreshBranches,
		checkoutBranch: (name: string) => checkout.mutate({ name }),
		checkoutPullRequest: (prNumber: number) => checkoutPr.mutate({ prNumber }),
		createBranch: (name: string) => createBranch.mutate({ name }),
	};
}
