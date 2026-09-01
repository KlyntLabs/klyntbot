import { type RefObject, useCallback, useEffect, useRef } from "react";
import { getCurrentWindow, isTauri } from "@/utils/tauri-bridge";

// Shrinks/grows the current Tauri window to the content's scrollHeight.
// ResizeObserver fires on layout-affecting changes (data load, mode switch,
// async-rendered widgets) — no MutationObserver needed.
export function useWindowAutoResize(
  contentRef: RefObject<HTMLDivElement | null>,
  opts: {
    width: number;
    maxHeight: number;
    minHeight?: number;
    // Pixels of vertical chrome outside the observed element (e.g. drag
    // handle) that should be added when sizing the window.
    extra?: number;
  },
) {
  const lastHeight = useRef(0);
  const rafId = useRef(0);
  const minHeight = opts.minHeight ?? 0;
  const extra = opts.extra ?? 0;

  const syncSize = useCallback(() => {
    if (!isTauri()) return;
    const el = contentRef.current;
    if (!el) return;
    const raw = Math.ceil(el.scrollHeight) + extra;
    const h = Math.min(Math.max(raw, minHeight), opts.maxHeight);
    if (h === lastHeight.current || h <= 0) return;
    lastHeight.current = h;
    getCurrentWindow()
      .setSize(opts.width, h)
      .catch((e: unknown) => console.error("window resize failed", e));
  }, [contentRef, opts.width, opts.maxHeight, minHeight, extra]);

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
