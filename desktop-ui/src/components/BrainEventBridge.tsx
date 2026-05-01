import { useEffect } from "react";
import { isTauri } from "@/utils/tauri-bridge";

const DEV_SSE_BASE = "http://127.0.0.1:3456";

// Bridges the dev HTTP server's `/api/brain/events` SSE stream to `window`
// CustomEvents. Tauri's `listen()` shim in `@/utils/tauri-bridge` falls back
// to `window.addEventListener` in browser mode, so once this bridge is mounted
// every existing `useEvent`/`listen` consumer (approval queue, brain ambient,
// provider degraded, entity updated, etc.) works transparently in Chrome.
//
// In a real Tauri webview, `__TAURI_INTERNALS__` is defined → this component
// short-circuits and Tauri's native event plugin handles delivery instead.
export function BrainEventBridge() {
  useEffect(() => {
    if (isTauri()) return;
    if (typeof window === "undefined" || typeof EventSource === "undefined") return;

    const es = new EventSource(`${DEV_SSE_BASE}/api/brain/events`);

    // SSE named events arrive as MessageEvents on the EventSource — we have
    // to register a listener per event name. The set is fixed by the BE
    // (see `streaming.rs::global_sse_handler` + every `emit_event` call).
    const eventNames = [
      "agent:approval_requested",
      "agent:approval_resolved",
      "agent:sandbox_policy_applied",
      "brain:ambient",
      "provider:degraded",
      "entity:updated",
      "focus:state",
      "voice:event",
    ];

    const listeners: Array<[string, EventListener]> = [];
    for (const name of eventNames) {
      const fn = (raw: MessageEvent) => {
        let detail: unknown = raw.data;
        try {
          detail = JSON.parse(raw.data);
        } catch {
          // pass through as string
        }
        window.dispatchEvent(new CustomEvent(name, { detail }));
      };
      es.addEventListener(name, fn as EventListener);
      listeners.push([name, fn as EventListener]);
    }

    es.onerror = () => {
      // EventSource auto-reconnects; we just log once for visibility.
      console.warn("[BrainEventBridge] SSE error — auto-reconnecting…");
    };

    return () => {
      for (const [name, fn] of listeners) es.removeEventListener(name, fn);
      es.close();
    };
  }, []);

  return null;
}
