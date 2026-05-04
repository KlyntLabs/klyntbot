import { invoke } from "../client";

export async function getCollaborationModes(workspaceId: string) {
  return invoke<unknown>("collaboration_mode_list", { workspaceId });
}
