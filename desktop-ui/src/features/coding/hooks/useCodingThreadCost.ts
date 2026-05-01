import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function useCodingThreadCost(threadId: string | null) {
  const [cost, setCost] = useState(0);
  const [tokens, setTokens] = useState(0);

  useEffect(() => {
    if (!threadId) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      const meta = (await invoke("chat_get_thread_meta", { sessionKey: threadId })) as {
        totalCostUsd?: number;
        totalTokens?: number;
      };
      if (!active) return;
      setCost(meta.totalCostUsd ?? 0);
      setTokens(meta.totalTokens ?? 0);
      unlisten = await listen<{ thread_id: string; cost_usd: number; tokens: number }>(
        "agent:provider_call",
        (e) => {
          if (e.payload.thread_id === threadId) {
            setCost((c) => c + e.payload.cost_usd);
            setTokens((t) => t + e.payload.tokens);
          }
        },
      );
      if (!active && unlisten) {
        unlisten();
        unlisten = undefined;
      }
    })();
    return () => {
      active = false;
      unlisten?.();
    };
  }, [threadId]);

  return { cost, tokens };
}
