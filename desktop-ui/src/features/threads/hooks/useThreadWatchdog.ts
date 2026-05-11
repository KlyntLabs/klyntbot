import { useEffect, useRef } from "react";

const WATCHDOG_TIMEOUT_MS = 90_000;

type Args = {
  threadId: string | null;
  isProcessing: boolean;
  onFire: (threadId: string) => void;
};

/**
 * Assistant-mode watchdog. Mirrors the coding-mode 90s heartbeat from
 * ThreadEventBuffer.ts. If no event arrives within 90s while isProcessing
 * is true, fire `onFire` and let the caller reset state.
 */
export function useThreadWatchdog({ threadId, isProcessing, onFire }: Args): void {
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
  }, [threadId, isProcessing, onFire]);
}
