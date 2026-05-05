import type { SkillOption } from "@/types";
import { invoke } from "../client";

export async function getSkillsList(workspaceId: string): Promise<SkillOption[]> {
  return invoke<SkillOption[]>("skills_list", { workspaceId });
}
