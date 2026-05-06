import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { listActiveSubagents, cancelSubagent } from "@/api/endpoints/subagent";
import type { SubagentEvent, SubagentActiveSummary } from "@/bindings";

export function useSubagents(threadId: string) {
  const [active, setActive] = useState<SubagentActiveSummary[]>([]);

  useEffect(() => {
    // Refuse to fire on non-coding thread IDs — `listActiveSubagents` would
    // otherwise reject with an unobserved promise on `ws-…` / `chat:…` IDs.
    if (!threadId.startsWith("coding:")) return;
    let cancelled = false;
    listActiveSubagents(threadId).then((s) => { if (!cancelled) setActive(s); });

    // Spawned events are routed to a thread-specific channel; all other
    // lifecycle events flow through the global channel.
    const unlistenSpawned = listen<SubagentEvent>(
      `agent:subagent_event#${threadId}`,
      (e) => { if (!cancelled) setActive((prev) => applySubagentEvent(prev, e.payload)); },
    );
    const unlistenGlobal = listen<SubagentEvent>(
      "agent:subagent_event",
      (e) => { if (!cancelled) setActive((prev) => applySubagentEvent(prev, e.payload)); },
    );

    return () => {
      cancelled = true;
      unlistenSpawned.then((fn) => fn());
      unlistenGlobal.then((fn) => fn());
    };
  }, [threadId]);

  const cancel = useCallback((agentId: string) => cancelSubagent(agentId), []);

  return { active, cancel };
}

export function applySubagentEvent(
  prev: SubagentActiveSummary[],
  e: SubagentEvent,
): SubagentActiveSummary[] {
  switch (e.kind) {
    case "spawned":
      return [...prev, {
        agentId: e.agent_id, label: e.label, profile: e.profile,
        iteration: 0, status: "running", startedAt: e.spawned_at,
        lastTool: null, durationMs: 0,
      }];
    case "progress":
      return prev.map((s) => s.agentId === e.agent_id
        ? { ...s, iteration: e.iteration, lastTool: e.last_tool ?? null }
        : s);
    case "completed":
    case "cancelled":
      return prev.filter((s) => s.agentId !== e.agent_id);
    default:
      return prev;
  }
}
