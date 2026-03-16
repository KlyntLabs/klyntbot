import { useCallback, useEffect, useState } from "react";

export function useFocusSession(focusedAt: string | null | undefined) {
  const [elapsedSecs, setElapsedSecs] = useState(0);

  useEffect(() => {
    if (!focusedAt) {
      setElapsedSecs(0);
      return;
    }

    const startMs = new Date(focusedAt).getTime();
    const tick = () => setElapsedSecs(Math.floor((Date.now() - startMs) / 1000));

    tick(); // immediate sync on mount / focusedAt change
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [focusedAt]);

  const formatElapsed = useCallback((secs: number): string => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    const mm = String(m).padStart(2, "0");
    const ss = String(s).padStart(2, "0");
    return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
  }, []);

  return { isActive: !!focusedAt, elapsedSecs, formatElapsed };
}
