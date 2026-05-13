import { useEffect, useRef, useState } from "react";
import { invoke } from "@/api/client";

interface AttachHandle {
  wsUrl: string;
  rows: number;
  cols: number;
  tailB64: string;
}

interface UseAttachSessionArgs {
  threadId: string;
  jobId: string;
  enabled: boolean;
}

interface UseAttachSessionResult {
  ws: WebSocket | null;
  handle: AttachHandle | null;
  error: string | null;
}

export function useAttachSession({
  threadId,
  jobId,
  enabled,
}: UseAttachSessionArgs): UseAttachSessionResult {
  const [handle, setHandle] = useState<AttachHandle | null>(null);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const [wsState, setWsState] = useState<WebSocket | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    (async () => {
      try {
        const h = await invoke<AttachHandle>("coding_job_attach", {
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
  }, [threadId, jobId, enabled]);

  return { ws: wsState, handle, error };
}
