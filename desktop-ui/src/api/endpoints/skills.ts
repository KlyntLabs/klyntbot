import { invoke } from "../client";

export async function getSkillsList(workspaceId: string) {
  return invoke<any>("skills_list", { workspaceId });
}
