import { getGitRemote } from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { WorkspaceInfo } from "@/types";

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
