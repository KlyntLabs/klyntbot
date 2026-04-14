import { useQuery } from "@shared/hooks/useQuery";
import type { InstalledSkill } from "@shared/types";

export function useSkillList() {
  return useQuery<InstalledSkill[]>("skill_list", {}, [], {
    invalidateOn: ["skills:updated"],
    staleTime: 30_000,
  });
}
