import { useCoachingNudge } from "@shared/hooks/useCoachingNudge";
import { Check, Lightbulb, X, XCircle } from "lucide-react";

const AUTO_COLLAPSE_MS = 60_000;

/**
 * Subtle coaching nudge banner shown above the chat input.
 * Shows the latest pending intervention with feedback buttons.
 * Auto-collapses after 60s if ignored.
 */
export function CoachingNudge({ isStreaming }: { isStreaming: boolean }) {
  const { nudge, handleFeedback } = useCoachingNudge({ autoCollapseMs: AUTO_COLLAPSE_MS });

  // Don't show while AI is streaming (queue until done)
  if (isStreaming) return null;
  if (!nudge) return null;

  return (
    <div className="px-6">
      <div className="max-w-3xl mx-auto">
        <div
          className="flex items-start gap-3 px-4 py-3 rounded-xl bg-[var(--glass-tint-info)] border border-[var(--glass-border)] backdrop-blur-sm"
          style={{ animation: "nudge-slide-in 0.25s ease-out" }}
        >
          <Lightbulb className="w-4 h-4 text-info shrink-0 mt-0.5" strokeWidth={1.5} />
          <p className="flex-1 text-[13px] text-secondary font-light leading-relaxed">
            {nudge.message}
          </p>
          <div className="flex items-center gap-1 shrink-0">
            <button
              type="button"
              onClick={() => handleFeedback(nudge.id, "helpful")}
              title="Helpful"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-success hover:bg-white/[0.06] transition-colors"
            >
              <Check className="w-3.5 h-3.5" strokeWidth={2} />
            </button>
            <button
              type="button"
              onClick={() => handleFeedback(nudge.id, "dismissed")}
              title="Dismiss"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <X className="w-3.5 h-3.5" strokeWidth={2} />
            </button>
            <button
              type="button"
              onClick={() => handleFeedback(nudge.id, "stop")}
              title="Stop suggesting this"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-destructive hover:bg-white/[0.06] transition-colors"
            >
              <XCircle className="w-3.5 h-3.5" strokeWidth={2} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
