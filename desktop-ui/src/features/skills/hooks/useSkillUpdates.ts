import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { AvailableVersion, InstalledSkill, UpgradePlan } from "@shared/types";
import { emitSkillsUpdated } from "../lib/emit";

export function useSkillCheckUpdates(name: string | undefined) {
  return useQuery<AvailableVersion[]>("skill_check_updates", name ? { name } : null, [], {
    staleTime: 300_000,
  });
}

export function useSkillUpgradePreview() {
  return useMutation<UpgradePlan, { name: string; targetSha: string }>("skill_upgrade_preview");
}

export function useSkillUpgradeApply() {
  const { mutate: raw, loading } = useMutation<InstalledSkill, { plan: UpgradePlan }>(
    "skill_upgrade_apply",
  );
  const mutate = async (plan: UpgradePlan) => {
    const out = await raw({ plan });
    if (out) emitSkillsUpdated();
    return out;
  };
  return { mutate, loading };
}
