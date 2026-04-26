import { getSkillsList } from "@services/tauri";
import { useCallback } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { SkillOption, WorkspaceInfo } from "@/types";

export function useSkills(activeWorkspace: WorkspaceInfo | null) {
  const workspaceId = activeWorkspace?.id ?? "";

  const query = useTauriQuery<SkillOption[]>({
    queryKey: qk.skills.list(workspaceId),
    queryFn: async () => {
      if (!activeWorkspace) return [];
      const list = await getSkillsList(activeWorkspace.id);
      return list.filter((s: any) => Boolean(s.name));
    },
    fallback: [],
    enabled: activeWorkspace !== null,
  });

  const refreshSkills = useCallback(async () => {
    await query.refetch();
  }, [query]);

  return {
    skills: query.data,
    refreshSkills,
  };
}
