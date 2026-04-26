import { getAppsList } from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { AppOption, WorkspaceInfo } from "@/types";

interface UseAppsArgs {
  activeWorkspace: WorkspaceInfo | null;
  activeThreadId: string | null;
}

export function useApps({ activeWorkspace, activeThreadId }: UseAppsArgs) {
  const workspaceId = activeWorkspace?.id ?? "";

  const query = useTauriQuery<AppOption[]>({
    queryKey: qk.apps.list(workspaceId, activeThreadId),
    queryFn: async () => {
      if (!activeWorkspace) return [];
      const list = await getAppsList(activeWorkspace.id, null, 100, activeThreadId);
      return list.filter((a: any) => Boolean(a.id) && Boolean(a.name));
    },
    fallback: [],
    enabled: activeWorkspace !== null,
  });

  const refreshApps = useCallback(async () => {
    await query.refetch();
  }, [query]);

  return {
    apps: query.data,
    refreshApps,
  };
}
