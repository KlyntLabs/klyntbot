import { getGitLog } from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { GitLogEntry, WorkspaceInfo } from "@/types";

interface GitLogState {
  entries: GitLogEntry[];
  total: number;
  ahead: number;
  behind: number;
  aheadEntries: GitLogEntry[];
  behindEntries: GitLogEntry[];
  upstream: string | null;
}

const emptyState: GitLogState = {
  entries: [],
  total: 0,
  ahead: 0,
  behind: 0,
  aheadEntries: [],
  behindEntries: [],
  upstream: null,
};

export function useGitLog(activeWorkspace: WorkspaceInfo | null) {
  const workspaceId = activeWorkspace?.id ?? "";

  const query = useTauriQuery<GitLogState>({
    queryKey: qk.git.log(workspaceId),
    queryFn: async () => {
      if (!activeWorkspace) return emptyState;
      return await getGitLog(activeWorkspace.id);
    },
    fallback: emptyState,
    enabled: activeWorkspace !== null,
    staleTime: 10_000,
  });

  const refresh = useCallback(async () => {
    await query.refetch();
  }, [query]);

  return {
    ...query.data,
    isLoading: query.isLoading,
    error: query.error == null ? null : String(query.error),
    refresh,
  };
}
