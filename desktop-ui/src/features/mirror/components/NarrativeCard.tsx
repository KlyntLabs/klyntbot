import { useMutation } from "@shared/hooks/useMutation";
import { ThumbsDown, ThumbsUp } from "lucide-react";
import { useState } from "react";

export interface TrendNarrative {
  id: string;
  fullNarrative: string;
  generatedAt: string;
  periodStart: string;
  periodEnd: string;
}

interface NarrativeCardProps {
  narrative: TrendNarrative | null | undefined;
}

export function NarrativeCard({ narrative }: NarrativeCardProps) {
  const { mutate: submitFeedback, loading } = useMutation<void, Record<string, unknown>>(
    "submit_mirror_feedback",
  );
  const [feedbackSent, setFeedbackSent] = useState<"helpful" | "not_helpful" | null>(null);

  const handleFeedback = async (feedback: "helpful" | "not_helpful") => {
    if (!narrative || feedbackSent || loading) return;
    await submitFeedback({
      itemId: narrative.id,
      target: "Narrative",
      feedback: feedback === "helpful" ? "Helpful" : "NotHelpful",
    });
    setFeedbackSent(feedback);
  };

  if (!narrative) {
    return (
      <div className="island rounded-xl p-5">
        <h2 className="text-ui font-medium text-fg-secondary mb-2">Weekly Reflection</h2>
        <p className="text-ui-xs text-fg-secondary">
          Your first weekly reflection will appear after 7 days of use.
        </p>
      </div>
    );
  }

  return (
    <div className="island rounded-xl p-5">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-ui font-medium text-fg-secondary">Weekly Reflection</h2>
        <span className="text-ui-xs text-fg-dim">
          {new Date(narrative.periodStart).toLocaleDateString()}
        </span>
      </div>
      <p className="text-ui-sm text-fg leading-relaxed">{narrative.fullNarrative}</p>
      <div className="flex items-center gap-2 mt-4 pt-3 border-t border-separator">
        <span className="text-ui-xs text-fg-dim mr-1">Was this helpful?</span>
        {feedbackSent ? (
          <span className="text-ui-xs text-fg-secondary">
            {feedbackSent === "helpful" ? "Thanks for the feedback!" : "Got it, we'll improve."}
          </span>
        ) : (
          <>
            <button
              type="button"
              onClick={() => handleFeedback("helpful")}
              disabled={loading}
              className="flex items-center gap-1 px-2 py-1 rounded text-ui-xs text-fg-secondary hover:text-status-success hover:bg-status-success/10 transition-colors disabled:opacity-50"
            >
              <ThumbsUp className="size-3" />
              Helpful
            </button>
            <button
              type="button"
              onClick={() => handleFeedback("not_helpful")}
              disabled={loading}
              className="flex items-center gap-1 px-2 py-1 rounded text-ui-xs text-fg-secondary hover:text-status-danger hover:bg-status-danger/10 transition-colors disabled:opacity-50"
            >
              <ThumbsDown className="size-3" />
              Not helpful
            </button>
          </>
        )}
      </div>
    </div>
  );
}
