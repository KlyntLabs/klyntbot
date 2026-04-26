import { isTauri } from "@shared/lib/utils";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

/**
 * Subscribe to a Tauri event, auto-cleanup on unmount.
 * No-ops in browser dev mode where Tauri IPC is unavailable.
 *
 * The `cancelled` guard inside the handler callback prevents stale listeners
 * from processing events during React StrictMode's double-mount cycle. Without
 * it, the brief window where both the old and new Tauri listeners are active
 * causes each event to be handled twice (duplicated text, tool calls, etc.).
 */
export function useEvent<T>(event: string, handler: (payload: T) => void) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (isTauri) {
      let cancelled = false;
      let unlisten: UnlistenFn | undefined;

      listen<T>(event, (e) => {
        if (!cancelled) handlerRef.current(e.payload);
      }).then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });

      return () => {
        cancelled = true;
        unlisten?.();
      };
    }

    // Browser dev mode: listen for CustomEvent on window
    const onCustom = (e: Event) => {
      handlerRef.current((e as CustomEvent).detail as T);
    };
    window.addEventListener(event, onCustom);
    return () => window.removeEventListener(event, onCustom);
  }, [event]);
}
