import { useEffect, useRef } from "react";
import { useAttachSession } from "@/features/coding/hooks/useAttachSession";

interface Props {
  jobId: string;
}

export function AttachTerminal({ jobId }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const { ws, handle, error } = useAttachSession({
    jobId,
    enabled: true,
  });

  useEffect(() => {
    if (!ws || !handle || !ref.current) return;
    let cancelled = false;
    let cleanup: (() => void) | null = null;

    (async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([
        import("@xterm/xterm"),
        import("@xterm/addon-fit"),
      ]);
      if (cancelled || !ref.current) return;
      const term = new Terminal({
        fontFamily: 'var(--ff-mono, "SF Mono", monospace)',
        fontSize: 13.5,
        cursorBlink: true,
        rows: handle.rows,
        cols: handle.cols,
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(ref.current);
      fit.fit();

      // Prime with last 4 KB of ring tail.
      try {
        term.write(atob(handle.tailB64));
      } catch {
        /* ignore decode error */
      }

      ws.onmessage = (e) => {
        if (e.data instanceof ArrayBuffer) {
          term.write(new Uint8Array(e.data));
        } else if (typeof e.data === "string") {
          term.write(e.data);
        }
      };
      ws.onclose = () => term.write("\r\n[detached]\r\n");
      ws.onerror = () => term.write("\r\n[connection error]\r\n");

      term.onData((s) => {
        if (ws.readyState === WebSocket.OPEN) ws.send(s);
      });
      term.onResize(({ rows, cols }) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ kind: "resize", rows, cols }));
        }
      });

      cleanup = () => term.dispose();
    })();

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [ws, handle]);

  if (error) {
    return (
      <div className="coding-jobs-panel__attach-error">
        Attach failed: {error}
      </div>
    );
  }
  return <div className="coding-jobs-panel__attach-term" ref={ref} />;
}
