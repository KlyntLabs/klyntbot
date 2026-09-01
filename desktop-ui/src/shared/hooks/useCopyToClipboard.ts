import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Manages clipboard copy with a self-resetting "copied" indicator.
 * Clears stale timers on rapid re-clicks and on unmount.
 */
export function useCopyToClipboard(duration = 2000) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  const copy = useCallback(
    async (text: string): Promise<boolean> => {
      try {
        await navigator.clipboard.writeText(text);
        clearTimeout(timerRef.current);
        setCopied(true);
        timerRef.current = setTimeout(() => setCopied(false), duration);
        return true;
      } catch {
        return false;
      }
    },
    [duration],
  );

  return { copied, copy };
}
