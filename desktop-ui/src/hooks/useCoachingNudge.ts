import { useCallback, useEffect, useRef, useState } from "react";
import type { CoachingIntervention } from "../lib/types";
import { useMutation } from "./useMutation";
import { useQuery } from "./useQuery";

export type CoachingFeedbackResponse = "helpful" | "dismissed" | "stop";

interface UseCoachingNudgeOptions {
  /** Milliseconds before an unseen nudge auto-collapses and reports ignored. */
  autoCollapseMs: number;
  /** Poll interval in ms (default 5000). */
  pollMs?: number;
}

interface UseCoachingNudgeResult {
  /** The current nudge to display, or null if nothing to show. */
  nudge: CoachingIntervention | null;
  /** Submit explicit feedback (helpful / dismissed / stop). */
  handleFeedback: (interventionId: string, response: CoachingFeedbackResponse) => Promise<void>;
}

/**
 * Shared coaching nudge state machine — polling, dismissal tracking,
 * auto-collapse timer, and feedback submission.
 *
 * Used by both the chat nudge (CoachingNudge.tsx) and tray nudge (SystemTray.tsx).
 */
export function useCoachingNudge({
  autoCollapseMs,
  pollMs = 5_000,
}: UseCoachingNudgeOptions): UseCoachingNudgeResult {
  const { data: interventions, refetch } = useQuery<CoachingIntervention[]>(
    "coaching_pending_interventions",
    undefined,
    [],
  );
  const { mutate: submitFeedback } = useMutation("coaching_submit_feedback");
  const { mutate: reportIgnored } = useMutation("coaching_report_ignored");

  const [dismissed, setDismissed] = useState<Set<string>>(() => new Set());
  const [collapsed, setCollapsed] = useState(false);
  const collapseTimer = useRef<ReturnType<typeof setTimeout>>();

  // Find the first non-dismissed intervention
  const current = interventions.find((i) => !dismissed.has(i.id)) ?? null;
  const currentId = current?.id;

  // Poll for new interventions
  useEffect(() => {
    const id = setInterval(refetch, pollMs);
    return () => clearInterval(id);
  }, [refetch, pollMs]);

  // Auto-collapse timer — reports ignored on expiry
  useEffect(() => {
    if (!currentId) return;
    setCollapsed(false);
    clearTimeout(collapseTimer.current);
    collapseTimer.current = setTimeout(() => {
      setCollapsed(true);
      reportIgnored({ intervention_id: currentId });
    }, autoCollapseMs);
    return () => clearTimeout(collapseTimer.current);
  }, [currentId, autoCollapseMs, reportIgnored]);

  const handleFeedback = useCallback(
    async (interventionId: string, response: CoachingFeedbackResponse) => {
      setDismissed((prev) => new Set(prev).add(interventionId));
      await submitFeedback({ intervention_id: interventionId, response });
      refetch();
    },
    [submitFeedback, refetch],
  );

  const nudge = current && !collapsed ? current : null;

  return { nudge, handleFeedback };
}
