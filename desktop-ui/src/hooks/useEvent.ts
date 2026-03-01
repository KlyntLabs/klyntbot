import { useEffect } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/**
 * Subscribe to a Tauri event, auto-cleanup on unmount.
 * Usage: useEvent<ContentChunkPayload>("agent:content_chunk", (payload) => { ... })
 * Not called yet — will be used for streaming chat responses.
 */
export function useEvent<T>(event: string, handler: (payload: T) => void) {
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    listen<T>(event, (e) => handler(e.payload)).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [event, handler]);
}
