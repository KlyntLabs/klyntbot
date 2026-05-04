import { useCallback, useState } from "react";
import { invoke } from "@/api/client";
import type { AgentsMdSource } from "@/bindings";

export function useAgentsMd(threadId: string, initialSources: AgentsMdSource[]) {
  const [sources, setSources] = useState<AgentsMdSource[]>(initialSources);
  const [refreshing, setRefreshing] = useState(false);
  const [lastRefreshedAt, setLastRefreshedAt] = useState<Date | null>(null);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const updated = await invoke<AgentsMdSource[]>(
        "coding_thread_refresh_agents_md", { threadId },
      );
      setSources(updated);
      setLastRefreshedAt(new Date());
    } finally {
      setRefreshing(false);
    }
  }, [threadId]);

  return { sources, refresh, refreshing, lastRefreshedAt };
}
