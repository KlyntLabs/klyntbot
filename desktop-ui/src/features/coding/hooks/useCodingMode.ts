import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/api/client";

export type CodingMode = "general" | "coding";

export function useCodingMode(threadId: string | null) {
  const [mode, setModeState] = useState<CodingMode>("general");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!threadId) return;
    let cancelled = false;
    (async () => {
      try {
        const session = (await invoke("chat_get_session", { sessionKey: threadId })) as {
          conversationType: string | null;
        };
        if (!cancelled) setModeState((session.conversationType as CodingMode) ?? "general");
      } catch (e) {
        console.warn("useCodingMode: chat_get_session failed", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [threadId]);

  useEffect(() => {
    if (!threadId) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<{ session_key: string; mode: CodingMode }>(
        "agent:mode_changed",
        (evt) => {
          if (evt.payload.session_key === threadId) {
            setModeState(evt.payload.mode);
          }
        },
      );
      if (!active && unlisten) {
        unlisten();
        unlisten = undefined;
      }
    })();
    return () => {
      active = false;
      unlisten?.();
    };
  }, [threadId]);

  // Session mode is immutable (set at creation time). setMode is a no-op.
  const setMode = useCallback(async (_next: CodingMode) => {
    console.warn("useCodingMode.setMode is a no-op — SessionMode is immutable");
  }, []);

  return { mode, setMode, loading };
}
