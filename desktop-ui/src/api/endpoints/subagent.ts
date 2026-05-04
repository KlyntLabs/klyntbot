import { invoke } from "@/api/client";
import type { SubagentActiveSummary, SubagentDetail } from "@/bindings";

export async function listActiveSubagents(threadId: string): Promise<SubagentActiveSummary[]> {
  return invoke<SubagentActiveSummary[]>("subagent_list_active", { threadId });
}
export async function cancelSubagent(agentId: string): Promise<void> {
  await invoke("subagent_cancel", { agentId });
}
export async function inspectSubagent(agentId: string): Promise<SubagentDetail> {
  return invoke<SubagentDetail>("subagent_inspect", { agentId });
}
