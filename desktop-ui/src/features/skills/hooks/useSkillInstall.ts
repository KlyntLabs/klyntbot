import { useMutation } from "@shared/hooks/useMutation";
import type { InstalledSkill, InstallPlan, UninstallMode } from "@shared/types";
import { emitSkillsUpdated } from "../lib/emit";

export function useSkillInstallPreview() {
  return useMutation<InstallPlan, { shorthand: string }>("skill_install_preview");
}

export function useSkillInstallApply() {
  const {
    mutate: raw,
    loading,
    error,
  } = useMutation<InstalledSkill, { plan: InstallPlan }>("skill_install_apply");
  const mutate = async (plan: InstallPlan) => {
    const out = await raw({ plan });
    if (out) emitSkillsUpdated();
    return out;
  };
  return { mutate, loading, error };
}

export function useSkillUninstall() {
  const { mutate: raw, loading } = useMutation<void, { name: string; mode: UninstallMode }>(
    "skill_uninstall",
  );
  const mutate = async (name: string, mode: UninstallMode) => {
    await raw({ name, mode });
    emitSkillsUpdated();
  };
  return { mutate, loading };
}

export function useSkillToggleEnabled() {
  const { mutate: raw, loading } = useMutation<void, { name: string; enabled: boolean }>(
    "skill_toggle_enabled",
  );
  const mutate = async (name: string, enabled: boolean) => {
    await raw({ name, enabled });
    emitSkillsUpdated();
  };
  return { mutate, loading };
}
