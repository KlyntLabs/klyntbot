import { invoke } from "@/api/client";
import type { RecallStats } from "@/bindings";

export async function fetchCodingRecallStats(
  workspaceId: string,
  days?: number,
): Promise<RecallStats> {
  return invoke<RecallStats>("coding_recall_stats", { workspaceId, days: days ?? null });
}
