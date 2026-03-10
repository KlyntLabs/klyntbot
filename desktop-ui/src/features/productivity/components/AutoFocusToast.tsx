import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import type { AutoFocusPayload } from "@shared/types";
import { useCallback, useState } from "react";
import { AppIcon, getAppColor } from "../lib/constants";

export function AutoFocusToast() {
  const [session, setSession] = useState<AutoFocusPayload | null>(null);
  const [confirming, setConfirming] = useState(false);

  useEvent<AutoFocusPayload>("focus:auto_detected", (payload) => {
    setSession(payload);
  });

  const handleConfirm = useCallback(async () => {
    if (!session) return;
    setConfirming(true);
    try {
      await ipc("productivity_auto_focus_confirm", { session });
    } finally {
      setSession(null);
      setConfirming(false);
    }
  }, [session]);

  const handleDismiss = useCallback(() => {
    if (!session) return;
    setSession(null);
  }, [session]);

  if (!session) return null;

  const color = getAppColor(session.dominantApp, null);
  const ratio = Math.round(session.productiveRatio * 100);

  return (
    <div className="col-span-3 animate-in fade-in slide-in-from-top-2 duration-300">
      <div
        className="glass-card p-4 flex items-center gap-3 border-l-2"
        style={{ borderLeftColor: "var(--success)" }}
      >
        <div className="flex-shrink-0">
          <AppIcon appName={session.dominantApp} color={color} />
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-[12px] font-medium text-primary">Focus session detected</span>
            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-success/15 text-success font-medium">
              {ratio}% productive
            </span>
          </div>
          <p className="text-[11px] font-light text-muted mt-0.5">
            {session.durationMins}min in {session.dominantApp}
          </p>
        </div>

        <div className="flex items-center gap-2 flex-shrink-0">
          <button
            type="button"
            onClick={handleConfirm}
            disabled={confirming}
            className="text-[11px] font-medium px-3 py-1.5 rounded-lg bg-success/15 text-success hover:bg-success/25 transition-colors disabled:opacity-50"
          >
            {confirming ? "Saving..." : "Confirm"}
          </button>
          <button
            type="button"
            onClick={handleDismiss}
            className="text-[11px] font-medium px-3 py-1.5 rounded-lg bg-[var(--surface-glass-subtle)] text-muted hover:bg-[var(--surface-glass-subtle-hover)] transition-colors"
          >
            Dismiss
          </button>
        </div>
      </div>
    </div>
  );
}
