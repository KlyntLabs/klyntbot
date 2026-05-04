import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function useCodingThreadCost(threadId: string | null) {
  const [cost, setCost] = useState(0);
  const [tokens, setTokens] = useState(0);

  useEffect(() => {
    if (!threadId) return;
    // Chat sessions are not coding threads; skip fetching cost to avoid
    // Storage not found backend noise.
    if (threadId.startsWith("chat:")) {
      setCost(0);
      setTokens(0);
      return;
    }
    let active = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      const thread = (await invoke("coding_thread_read", { threadId })) as {
        totalCostUsd?: number;
        totalTokens?: number;
      };
      if (!active) return;
      setCost(thread.totalCostUsd ?? 0);
      setTokens(thread.totalTokens ?? 0);
      unlisten = await listen<{
        threadId?: string | null;
        promptTokensDelta?: number;
        completionTokensDelta?: number;
        usdDelta?: number;
        threadTotalUsd?: number | null;
      }>("agent:cost_update", (e) => {
        const p = e.payload;
        if (p.threadId && p.threadId !== threadId) return;
        if (typeof p.threadTotalUsd === "number") {
          setCost(p.threadTotalUsd);
        } else if (typeof p.usdDelta === "number") {
          setCost((c) => c + (p.usdDelta ?? 0));
        }
        const tokDelta = (p.promptTokensDelta ?? 0) + (p.completionTokensDelta ?? 0);
        if (tokDelta) setTokens((t) => t + tokDelta);
      });
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
