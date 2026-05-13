import { useEffect, useRef, useState } from "react";
import { invoke } from "@/api/client";
import type { AttachResult } from "@/bindings";

interface UseAttachSessionArgs {
  jobId: string;
  enabled: boolean;
}

interface UseAttachSessionResult {
  ws: WebSocket | null;
  handle: AttachResult | null;
  error: string | null;
}

export function useAttachSession({
  jobId,
  enabled,
}: UseAttachSessionArgs): UseAttachSessionResult {
  const [handle, setHandle] = useState<AttachResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const [wsState, setWsState] = useState<WebSocket | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    (async () => {
      try {
        const h = await invoke<AttachResult>("coding_job_attach", {
          jobId,
        });
        if (cancelled) return;
        setHandle(h);
        const ws = new WebSocket(h.wsUrl);
        ws.binaryType = "arraybuffer";
        wsRef.current = ws;
        setWsState(ws);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();

    return () => {
      cancelled = true;
      const ws = wsRef.current;
      if (ws) {
        try {
          ws.close();
        } catch {
          /* ignore */
        }
      }
      invoke("coding_job_detach", { jobId }).catch(() => {
        /* ignore — bridge auto-detaches on WS close */
      });
    };
  }, [jobId, enabled]);

  return { ws: wsState, handle, error };
}
