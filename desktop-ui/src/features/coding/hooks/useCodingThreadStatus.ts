import { useMemo } from "react";
import { useRunningCodingIds } from "@/features/coding/state/ThreadEventBuffer";

type CodingStatus = {
  isProcessing: boolean;
};

/**
 * Per-thread "is a turn in flight?" map for coding threads, derived from
 * the global ThreadEventBuffer running set. Replaces the standalone
 * `agent:thread_event` listener that lived in this file pre-Phase 4.
 *
 * Keeps the same `Record<threadId, { isProcessing }>` shape so existing
 * call sites (the assistant-mode threadStatusById merge in MainApp) need
 * no further changes.
 */
export function useCodingThreadStatus(): Record<string, CodingStatus> {
  const running = useRunningCodingIds();
  return useMemo(() => {
    const map: Record<string, CodingStatus> = {};
    for (const id of running) map[id] = { isProcessing: true };
    return map;
  }, [running]);
}
