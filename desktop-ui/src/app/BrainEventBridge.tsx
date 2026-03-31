/** Forwards dev-server SSE events as CustomEvents so useEvent works in browser mode. */
import { DEV_SSE_BASE, isTauri } from "@shared/lib/utils";
import { useEffect } from "react";

/** Event names to subscribe to on the global SSE stream. */
const GLOBAL_SSE_EVENTS = [
  "brain:ambient",
  "provider:degraded",
  "entity:updated",
  "focus:state_changed",
  "chat:thread_created",
  "chat:thread_updated",
  "chat:message_added",
] as const;

export function BrainEventBridge() {
  useEffect(() => {
    if (isTauri) return;

    const source = new EventSource(`${DEV_SSE_BASE}/api/brain/events`);

    for (const eventName of GLOBAL_SSE_EVENTS) {
      source.addEventListener(eventName, (e: MessageEvent) => {
        try {
          const payload = JSON.parse(e.data);
          window.dispatchEvent(new CustomEvent(eventName, { detail: payload }));
        } catch {
          // Ignore malformed SSE frames
        }
      });
    }

    return () => source.close();
  }, []);

  return null;
}
