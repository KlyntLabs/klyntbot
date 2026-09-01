// Stubs for features that haven't been ported into the new desktop-ui yet:
// `@features/coaching/MicroReviewPrompt`, `@features/autotuner/useAutoTunerStatus`,
// and `@shared/hooks/useCoachingNudge`.

export interface CoachingNudge {
  id: string;
  message: string;
}

export interface UseCoachingNudgeOpts {
  autoCollapseMs?: number;
}

export function useCoachingNudge(_opts?: UseCoachingNudgeOpts) {
  return {
    nudge: null as CoachingNudge | null,
    handleFeedback: (_id: string, _kind: "helpful" | "dismissed" | "stop") => {},
  };
}

export interface AutoTunerStatus {
  champion: { days_active: number } | null;
}

export function useAutoTunerStatus() {
  return { data: null as AutoTunerStatus | null };
}

interface MicroReviewPromptProps {
  dueCount: number;
  onAccept: () => void;
  onSkip: () => void;
}

export function MicroReviewPrompt({ dueCount, onAccept, onSkip }: MicroReviewPromptProps) {
  if (dueCount <= 0) return null;
  return (
    <div className="tray-review-prompt">
      <span className="tray-muted-xs">{dueCount} cards due</span>
      <div className="tray-review-prompt-actions">
        <button type="button" className="tray-pill is-brand" onClick={onAccept}>
          Review
        </button>
        <button type="button" className="tray-pill" onClick={onSkip}>
          Skip
        </button>
      </div>
    </div>
  );
}
