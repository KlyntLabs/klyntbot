import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

type CodingStatus = {
  isProcessing: boolean;
  hasUnread: boolean;
  isReviewing: boolean;
  processingStartedAt: number | null;
  lastDurationMs: number | null;
};

/// Tracks per-thread "is a turn in flight?" for coding threads by listening to
/// the `agent:thread_event` stream globally. The legacy `threadStatusById`
/// map (in `useThreads`) is fed by the assistant-mode `app-server-event`
/// pipeline only — without this bridge, coding turns set isProcessing=true
/// somewhere upstream but never flip it back, leaving the composer's queue
/// gate stuck open and silently buffering every subsequent message.
export function useCodingThreadStatus() {
  const [statusById, setStatusById] = useState<Record<string, CodingStatus>>({});

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    listen<{ kind: string; thread_id?: string }>("agent:thread_event", (e) => {
      if (cancelled) return;
      const evt = e.payload;
      const threadId = evt.thread_id;
      if (!threadId) return;
      if (evt.kind === "turn_started") {
        setStatusById((prev) => ({
          ...prev,
          [threadId]: {
            isProcessing: true,
            hasUnread: false,
            isReviewing: false,
            processingStartedAt: Date.now(),
            lastDurationMs: prev[threadId]?.lastDurationMs ?? null,
          },
        }));
      } else if (evt.kind === "turn_completed") {
        setStatusById((prev) => ({
          ...prev,
          [threadId]: {
            isProcessing: false,
            hasUnread: false,
            isReviewing: false,
            processingStartedAt: null,
            lastDurationMs:
              prev[threadId]?.processingStartedAt != null
                ? Date.now() - prev[threadId].processingStartedAt!
                : (prev[threadId]?.lastDurationMs ?? null),
          },
        }));
      }
    }).then((un) => {
      if (cancelled) un();
      else unlisten = un;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return statusById;
}
