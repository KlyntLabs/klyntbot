import type { InstalledSkill } from "@shared/types";
import { useMemo } from "react";
import { useSkillList } from "./useSkillList";

export function useSkillDetail(name: string | undefined) {
  const { data: all } = useSkillList();
  const installed: InstalledSkill | undefined = useMemo(
    () => all?.find((s) => s.name === name),
    [all, name],
  );
  return { installed };
}
