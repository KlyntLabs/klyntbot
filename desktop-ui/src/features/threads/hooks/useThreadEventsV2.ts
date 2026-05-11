import { listen, type Event as TauriEvent } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import type { ThreadEvent } from "@/bindings";

type Handler = (event: ThreadEvent) => void;

/**
 * Listen to the unified `thread:event` v2 channel.
 *
 * Replaces the 50+ individual `agent:*` listeners with a single typed channel.
 * Generation filtering is the caller's responsibility.
 */
export function useThreadEventsV2(
  sessionKey: string | null,
  onEvent: Handler,
): void {
  const handlerRef = useRef(onEvent);
  handlerRef.current = onEvent;

  useEffect(() => {
    if (!sessionKey) return;

    let unlisten: (() => void) | null = null;

    listen<ThreadEvent>("thread:event", (evt: TauriEvent<ThreadEvent>) => {
      if (evt.payload.session_key !== sessionKey) return;
      handlerRef.current(evt.payload);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, [sessionKey]);
}
