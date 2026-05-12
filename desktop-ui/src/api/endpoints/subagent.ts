import { invoke } from "../client";
import type { SubagentActiveSummary } from "@/bindings";

export type SubagentInstanceSummary = {
  agentId: string;
  sessionId: string;
  parentAgentId: string | null;
  description: string;
  status: string;
  turnsUsedTotal: number;
  lastCapHitAt: number | null;
  updatedAt: number;
};

export async function subagentListForSession(
  sessionId: string,
): Promise<SubagentInstanceSummary[]> {
  return invoke<SubagentInstanceSummary[]>("subagent_list_for_session", { sessionId });
}

export async function listActiveSubagents(threadId: string): Promise<SubagentActiveSummary[]> {
  return invoke<SubagentActiveSummary[]>("subagent_list_active", { threadId });
}

export async function cancelSubagent(agentId: string): Promise<void> {
  return invoke<void>("subagent_cancel", { agentId });
}
