import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { cn } from "@shared/lib/utils";
import { useEffect, useState } from "react";

interface DistractionDetectedPayload {
  sessionId: string;
  appName: string;
  previousApp: string;
  previousContext: string;
  reason: string;
}

/**
 * DistractionInterventionBanner — full-width overlay banner that appears when
 * distraction is detected during focus mode. Shows what app user drifted to
 * and offers options to resume focus, dismiss, snooze, or end session.
 *
 * Design: glassmorphic overlay with prominent action buttons and a dismissal timer.
 */
export function DistractionInterventionBanner() {
  const [distraction, setDistraction] = useState<DistractionDetectedPayload | null>(null);
  const [snoozeCountdown, setSnoozeCountdown] = useState(0);
  const [dismissed, setDismissed] = useState(false);

  // Listen for distraction detection events
  useEvent<DistractionDetectedPayload>("distraction:detected", (payload) => {
    setDistraction(payload);
    setSnoozeCountdown(0);
    setDismissed(false);
  });

  // Countdown timer for snooze
  useEffect(() => {
    if (snoozeCountdown <= 0 || dismissed) return;
    const timer = setTimeout(() => setSnoozeCountdown((c) => c - 1), 1000);
    return () => clearTimeout(timer);
  }, [snoozeCountdown, dismissed]);

  // Auto-dismiss after 30 seconds of no interaction
  useEffect(() => {
    if (!distraction || dismissed) return;
    const timer = setTimeout(() => setDismissed(true), 30_000);
    return () => clearTimeout(timer);
  }, [distraction, dismissed]);

  if (!distraction || dismissed) return null;

  const respond = async (action: string) => {
    try {
      await ipc("distraction_respond", {
        action,
        appName: distraction.appName,
      });
    } catch (error) {
      console.error("Failed to respond to distraction:", error);
    }
  };

  const handleBackToWork = async () => {
    await respond("back_to_work");
    setDismissed(true);
  };

  const handleNotDistraction = async () => {
    await respond("not_distraction");
    setDismissed(true);
  };

  const handleSnooze = async () => {
    await respond("snooze");
    setSnoozeCountdown(5 * 60);
  };

  const handleEndFocus = async () => {
    await respond("end_focus");
    setDismissed(true);
  };

  return (
    <div
      className={cn(
        "glass-panel mx-2 my-2 rounded-lg border border-border overflow-hidden transition-all duration-300",
        dismissed && "opacity-0 pointer-events-none",
      )}
      style={{
        backgroundColor: "var(--surface-base)",
      }}
    >
      {/* Left accent bar */}
      <div
        className="absolute left-0 top-0 bottom-0 w-1 rounded-full"
        style={{ backgroundColor: "var(--timeline-focus)" }}
      />

      <div className="px-4 py-3 pl-5 flex flex-col gap-3">
        {/* Header message */}
        <div className="flex flex-col gap-1">
          <p className="text-sm text-muted-foreground">Looks like you drifted from</p>
          <p className="text-base font-semibold text-foreground">
            {distraction.previousContext || distraction.previousApp}
          </p>
          {distraction.appName && (
            <p className="text-xs text-dim">Currently in: {distraction.appName}</p>
          )}
        </div>

        {/* Main actions grid — Back to work + Not a distraction */}
        <div className="flex gap-2">
          <button
            type="button"
            onClick={handleBackToWork}
            className={cn(
              "flex-1 px-3 py-2 rounded-md text-sm font-medium transition-colors",
              "bg-success/20 text-success border border-success/30 hover:bg-success/30",
            )}
          >
            Back to work
          </button>
          <button
            type="button"
            onClick={handleNotDistraction}
            className={cn(
              "flex-1 px-3 py-2 rounded-md text-sm font-medium transition-colors",
              "bg-accent text-muted-foreground border border-border hover:bg-muted",
            )}
          >
            Not a distraction
          </button>
        </div>

        {/* Secondary actions — Snooze + End focus */}
        <div className="flex gap-2 text-xs">
          <button
            type="button"
            onClick={handleSnooze}
            disabled={snoozeCountdown > 0}
            className={cn(
              "flex-1 px-2 py-1.5 rounded-md font-medium transition-colors",
              snoozeCountdown > 0
                ? "bg-card text-dim cursor-not-allowed"
                : "bg-accent text-muted-foreground hover:bg-muted border border-border",
            )}
          >
            {snoozeCountdown > 0 ? `${Math.ceil(snoozeCountdown / 60)}m snooze` : "5 more minutes"}
          </button>
          <button
            type="button"
            onClick={handleEndFocus}
            className={cn(
              "flex-1 px-2 py-1.5 rounded-md font-medium transition-colors",
              "text-muted-foreground hover:text-foreground hover:bg-accent",
            )}
          >
            End focus session
          </button>
        </div>
      </div>
    </div>
  );
}
