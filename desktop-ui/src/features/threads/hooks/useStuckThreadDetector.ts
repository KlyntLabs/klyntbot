import { useEffect, useState } from "react";

const STUCK_THRESHOLD_MS = 5_000;

export function useStuckThreadDetector(
  isProcessing: boolean,
  processingStartedAt: number | null,
): { isStuck: boolean; stuckDurationMs: number } {
  const [, setTick] = useState(0);

  useEffect(() => {
    if (!isProcessing || processingStartedAt == null) return;
    const interval = setInterval(() => setTick((n) => n + 1), 1_000);
    return () => clearInterval(interval);
  }, [isProcessing, processingStartedAt]);

  if (!isProcessing || processingStartedAt == null) {
    return { isStuck: false, stuckDurationMs: 0 };
  }

  const elapsed = Date.now() - processingStartedAt;
  return {
    isStuck: elapsed > STUCK_THRESHOLD_MS,
    stuckDurationMs: elapsed,
  };
}
