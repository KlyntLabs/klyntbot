import { useCallback, useState } from "react";
import { startReview } from "@/api/endpoints/review";
import type { ReviewResult } from "@/bindings";

export function useReview(threadId: string) {
  const [running, setRunning] = useState(false);
  const [lastResult, setLastResult] = useState<ReviewResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (target: string | null = null) => {
    setRunning(true); setError(null);
    try {
      const r = await startReview(threadId, target, "inline");
      setLastResult(r);
      return r;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg); throw e;
    } finally { setRunning(false); }
  }, [threadId]);

  return { run, running, lastResult, error };
}
