import { DEV_SSE_BASE, isTauri } from "@shared/lib/utils";
import { useEffect } from "react";

const INSIGHT_EVENTS = [
  "insight:synthesis-chunk",
  "insight:synthesis-done",
  "insight:tab-done",
  "insight:error",
  "insight:perspectives-meta",
  "insight:persona-perspective",
  "insight:changes-summary",
];

/**
 * In browser dev mode, connects to the dev server's insight SSE endpoint
 * and translates server-sent events into `CustomEvent`s on `window`.
 * This bridges the gap between backend `emit_event()` and `useEvent()`.
 *
 * No-ops in Tauri mode (events arrive natively).
 */
export function useInsightSSE(active: boolean) {
  useEffect(() => {
    if (!active || isTauri) return;
    const es = new EventSource(`${DEV_SSE_BASE}/api/insight/events`);
    const handler = (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        window.dispatchEvent(new CustomEvent(e.type, { detail: data }));
      } catch {
        // malformed SSE payload
      }
    };
    for (const evt of INSIGHT_EVENTS) {
      es.addEventListener(evt, handler);
    }
    return () => es.close();
  }, [active]);
}
