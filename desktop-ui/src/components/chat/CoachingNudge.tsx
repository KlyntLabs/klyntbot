import { Check, Lightbulb, X, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type { CoachingIntervention } from "../../lib/types";

const AUTO_COLLAPSE_MS = 60_000;

/**
 * Subtle coaching nudge banner shown above the chat input.
 * Shows the latest pending intervention with feedback buttons.
 * Auto-collapses after 60s if ignored.
 */
export function CoachingNudge({ isStreaming }: { isStreaming: boolean }) {
  const { data: interventions, refetch } = useQuery<CoachingIntervention[]>(
    "coaching_pending_interventions",
    undefined,
    [],
  );

  const { mutate: submitFeedback } = useMutation("coaching_submit_feedback");

  // Poll every 5s for new interventions
  useEffect(() => {
    const id = setInterval(refetch, 5_000);
    return () => clearInterval(id);
  }, [refetch]);

  // Track dismissed IDs locally so they disappear immediately
  const [dismissed, setDismissed] = useState<Set<string>>(() => new Set());

  // Auto-collapse timer
  const [collapsed, setCollapsed] = useState(false);
  const collapseTimer = useRef<ReturnType<typeof setTimeout>>();

  // Find the first non-dismissed intervention
  const current = interventions.find((i) => !dismissed.has(i.id));
  const currentId = current?.id;

  // Reset collapse timer when a new intervention appears
  useEffect(() => {
    if (!currentId) return;
    setCollapsed(false);
    clearTimeout(collapseTimer.current);
    collapseTimer.current = setTimeout(() => setCollapsed(true), AUTO_COLLAPSE_MS);
    return () => clearTimeout(collapseTimer.current);
  }, [currentId]);

  const handleFeedback = useCallback(
    async (interventionId: string, response: string) => {
      setDismissed((prev) => new Set(prev).add(interventionId));
      await submitFeedback({ intervention_id: interventionId, response });
      refetch();
    },
    [submitFeedback, refetch],
  );

  // Don't show while AI is streaming (queue until done)
  if (isStreaming) return null;
  if (!current || collapsed) return null;

  return (
    <div className="px-6">
      <div className="max-w-3xl mx-auto">
        <div
          className="flex items-start gap-3 px-4 py-3 rounded-xl bg-[var(--glass-tint-info)] border border-[var(--glass-border)] backdrop-blur-sm"
          style={{ animation: "nudge-slide-in 0.25s ease-out" }}
        >
          <Lightbulb className="w-4 h-4 text-info shrink-0 mt-0.5" strokeWidth={1.5} />
          <p className="flex-1 text-[13px] text-secondary font-light leading-relaxed">
            {current.message}
          </p>
          <div className="flex items-center gap-1 shrink-0">
            <button
              type="button"
              onClick={() => handleFeedback(current.id, "helpful")}
              title="Helpful"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-success hover:bg-white/[0.06] transition-colors"
            >
              <Check className="w-3.5 h-3.5" strokeWidth={2} />
            </button>
            <button
              type="button"
              onClick={() => handleFeedback(current.id, "dismissed")}
              title="Dismiss"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <X className="w-3.5 h-3.5" strokeWidth={2} />
            </button>
            <button
              type="button"
              onClick={() => handleFeedback(current.id, "stop")}
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
