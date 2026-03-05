import { useCallback, useEffect, useRef } from "react";

/** Returns a ref and onInput handler that auto-resizes a textarea to fit its content. */
export function useAutoResizeTextarea(input: string) {
  const ref = useRef<HTMLTextAreaElement>(null);

  const handleInput = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, []);

  // Reset height when input is cleared (e.g. after send)
  useEffect(() => {
    if (!input && ref.current) {
      ref.current.style.height = "auto";
    }
  }, [input]);

  return { ref, handleInput };
}
