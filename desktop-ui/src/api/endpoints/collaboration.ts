import { invoke } from "../client";

export async function getCollaborationModes(workspaceId: string) {
  return invoke<any>("collaboration_mode_list", { workspaceId });
}
