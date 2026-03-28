import { type RefObject, useEffect, useRef } from "react";

/**
 * Combined click-outside + Escape key dismiss handler.
 * Attaches both a `mousedown` listener (for clicks outside `ref`)
 * and a `keydown` listener (for Escape) that call `onDismiss`.
 * No-ops when `enabled` is false.
 */
export function useDismiss(
  ref: RefObject<HTMLElement | null>,
  onDismiss: () => void,
  enabled: boolean,
): void {
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    if (!enabled) return;

    function handleMouseDown(e: MouseEvent): void {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onDismissRef.current();
      }
    }

    function handleKeyDown(e: KeyboardEvent): void {
      if (e.key === "Escape") onDismissRef.current();
    }

    document.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [ref, enabled]);
}
