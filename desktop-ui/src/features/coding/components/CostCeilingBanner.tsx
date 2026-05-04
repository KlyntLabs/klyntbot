import { useEffect, useState } from "react";
import { subscribeCostUpdates, type CostUpdate } from "@/api/endpoints/cost";

export function CostCeilingBanner({ sessionKey }: { sessionKey: string }) {
  const [breach, setBreach] = useState<CostUpdate | null>(null);

  useEffect(() => {
    let mounted = true;
    const unlisten = subscribeCostUpdates((update) => {
      if (!mounted) return;
      if (update.ceilingBreached) {
        setBreach(update);
      }
    });
    return () => {
      mounted = false;
      void unlisten.then((fn) => fn());
    };
  }, [sessionKey]);

  if (!breach) return null;

  return (
    <div className="cost-ceiling-banner" role="alert">
      <span className="cost-ceiling-banner__text">
        Cost ceiling reached: ${breach.threadTotalUsd?.toFixed(2) ?? "??"} spent
      </span>
      <button
        className="cost-ceiling-banner__dismiss"
        onClick={() => setBreach(null)}
        aria-label="Dismiss cost alert"
      >
        ×
      </button>
    </div>
  );
}
