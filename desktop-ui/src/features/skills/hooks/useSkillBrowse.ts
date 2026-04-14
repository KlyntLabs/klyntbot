import { useQuery } from "@shared/hooks/useQuery";
import type { SkillBrowseRow } from "@shared/types";

export function useSkillBrowse(query: string | undefined) {
  return useQuery<SkillBrowseRow[]>("skill_browse", query ? { query } : {}, [], {
    invalidateOn: ["skills:updated"],
    staleTime: 60_000,
  });
}
