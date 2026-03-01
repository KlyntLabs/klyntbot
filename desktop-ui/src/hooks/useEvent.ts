import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;

/**
 * Subscribe to a Tauri event, auto-cleanup on unmount.
 * No-ops in browser dev mode where Tauri IPC is unavailable.
 */
export function useEvent<T>(event: string, handler: (payload: T) => void) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    listen<T>(event, (e) => handlerRef.current(e.payload)).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [event]);
}
