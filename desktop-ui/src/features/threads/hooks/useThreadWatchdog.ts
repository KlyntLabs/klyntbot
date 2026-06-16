import { useEffect, useRef } from "react";

const WATCHDOG_TIMEOUT_MS = 90_000;

type Args = {
  threadId: string | null;
  isProcessing: boolean;
  /** Timestamp (ms) of the last server heartbeat. Changing this resets the timer. */
  lastHeartbeatAt?: number | null;
  onFire: (threadId: string) => void;
};

/**
 * Assistant-mode watchdog. Mirrors the coding-mode 90s heartbeat from
 * ThreadEventBuffer.ts. If no event arrives within 90s while isProcessing
 * is true, fire `onFire` and let the caller reset state.
 *
 * When `lastHeartbeatAt` changes, the timer resets — this is wired to the
 * server-side `ThreadEvent::Heartbeat` emitted every 30s during active turns.
 */
export function useThreadWatchdog({ threadId, isProcessing, lastHeartbeatAt, onFire }: Args): void {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }

    if (!threadId || !isProcessing) return;

    timeoutRef.current = setTimeout(() => {
      console.warn(`[threads] watchdog fired for ${threadId} after ${WATCHDOG_TIMEOUT_MS}ms`);
      onFire(threadId);
    }, WATCHDOG_TIMEOUT_MS);

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };
  }, [threadId, isProcessing, lastHeartbeatAt, onFire]);
}
