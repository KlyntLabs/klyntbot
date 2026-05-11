import { useMemo } from "react";
import { useChatStore } from "@/features/threads/store/useChatStore";

type ThreadActivityStatus = {
  isProcessing: boolean;
  hasUnread: boolean;
  isReviewing: boolean;
  processingStartedAt: number | null;
  lastDurationMs: number | null;
};

/**
 * Per-thread "is a turn in flight?" map for coding threads, derived from
 * the global `useChatStore` running set. Replaces the standalone
 * `ThreadEventBuffer` sync-external-store hook.
 */
export function useCodingThreadStatus(): Record<string, ThreadActivityStatus> {
  const running = useChatStore((store) => store.codingRunningIds);
  return useMemo(() => {
    const map: Record<string, ThreadActivityStatus> = {};
    for (const id of running) {
      map[id] = {
        isProcessing: true,
        hasUnread: false,
        isReviewing: false,
        processingStartedAt: null,
        lastDurationMs: null,
      };
    }
    return map;
  }, [running]);
}
