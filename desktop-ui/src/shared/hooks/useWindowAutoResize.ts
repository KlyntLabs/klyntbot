import { isTauri } from "@shared/lib/utils";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type RefObject, useCallback, useEffect, useRef } from "react";

/**
 * Auto-resize a Tauri window to match its content element's height.
 * ResizeObserver fires on any layout-affecting change, including content
 * loads — no MutationObserver needed.
 */
export function useWindowAutoResize(
  contentRef: RefObject<HTMLDivElement | null>,
  opts: { width: number; maxHeight: number },
) {
  const lastHeight = useRef(0);
  const rafId = useRef(0);

  const syncSize = useCallback(() => {
    if (!isTauri) return;

    const el = contentRef.current;
    if (!el) return;

    const h = Math.min(Math.ceil(el.scrollHeight), opts.maxHeight);
    if (h === lastHeight.current || h <= 0) return;
    lastHeight.current = h;

    getCurrentWindow()
      .setSize(new LogicalSize(opts.width, h))
      .catch((e: unknown) => console.error("window resize failed", e));
  }, [contentRef, opts.width, opts.maxHeight]);

  const debouncedSync = useCallback(() => {
    cancelAnimationFrame(rafId.current);
    rafId.current = requestAnimationFrame(syncSize);
  }, [syncSize]);

  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;

    const ro = new ResizeObserver(() => debouncedSync());
    ro.observe(el);

    syncSize();
    return () => {
      cancelAnimationFrame(rafId.current);
      ro.disconnect();
    };
  }, [contentRef, syncSize, debouncedSync]);
}
