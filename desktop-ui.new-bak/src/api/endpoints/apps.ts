import type { AppOption } from "@/types";
import { invoke } from "../client";

export async function getAppsList(
  workspaceId: string,
  cursor?: string | null,
  limit?: number | null,
  threadId?: string | null,
): Promise<AppOption[]> {
  return invoke<AppOption[]>("apps_list", { workspaceId, cursor, limit, threadId });
}
