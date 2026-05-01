import { useEffect, useState } from "react";
import { commands } from "@/bindings";
import type { MirrorAlertRow } from "@/bindings";

export function CostCeilingBanner({ sessionKey }: { sessionKey: string }) {
  const [alert, setAlert] = useState<MirrorAlertRow | null>(null);

  useEffect(() => {
    let mounted = true;
    commands
      .codingMemoryMirrorAlertsFeed({
        kind: "costThresholdCrossed",
        severity: null,
        repo: null,
        limit: 20,
      })
      .then((res) => {
        if (!mounted || res.status !== "ok") return;
        const hit = res.data[0];
        if (hit) setAlert(hit);
      });
    return () => {
      mounted = false;
    };
  }, [sessionKey]);

  if (!alert) return null;

  return (
    <div className="cost-ceiling-banner" role="alert">
      <span className="cost-ceiling-banner__text">
        Cost alert: {alert.kind} — {alert.severity}
      </span>
      <button
        className="cost-ceiling-banner__dismiss"
        onClick={() => setAlert(null)}
        aria-label="Dismiss cost alert"
      >
        ×
      </button>
    </div>
  );
}
