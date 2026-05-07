import { useMemo } from "react";
import { useRunningCodingIds } from "@/features/coding/state/ThreadEventBuffer";

type ThreadActivityStatus = {
  isProcessing: boolean;
  hasUnread: boolean;
  isReviewing: boolean;
  processingStartedAt: number | null;
  lastDurationMs: number | null;
};

/**
 * Per-thread "is a turn in flight?" map for coding threads, derived from
 * the global ThreadEventBuffer running set. Replaces the standalone
 * `agent:thread_event` listener that lived in this file pre-Phase 4.
 *
 * Returns the full ThreadActivityStatus shape so the spread-merge with
 * assistant-mode threadStatusById in MainApp is type-safe.
 */
export function useCodingThreadStatus(): Record<string, ThreadActivityStatus> {
  const running = useRunningCodingIds();
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
