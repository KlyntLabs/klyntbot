import { useMutation } from "@shared/hooks/useMutation";
import { ThumbsDown, ThumbsUp } from "lucide-react";
import { useState } from "react";

export interface TrendNarrative {
  id: string;
  fullNarrative: string;
  generatedAt: string;
  weekOf: string;
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
      item_id: narrative.id,
      target: "narrative",
      feedback,
    });
    setFeedbackSent(feedback);
  };

  if (!narrative) {
    return (
      <div className="glass-card rounded-xl p-5">
        <h2 className="text-[13px] font-medium text-muted-foreground mb-2">Weekly Reflection</h2>
        <p className="text-[11px] text-muted-foreground">
          Your first weekly reflection will appear after 7 days of use.
        </p>
      </div>
    );
  }

  return (
    <div className="glass-card rounded-xl p-5">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-[13px] font-medium text-muted-foreground">Weekly Reflection</h2>
        <span className="text-2xs text-dim">Week of {narrative.weekOf}</span>
      </div>
      <p className="text-[12px] text-foreground leading-relaxed">{narrative.fullNarrative}</p>
      <div className="flex items-center gap-2 mt-4 pt-3 border-t border-border-subtle">
        <span className="text-2xs text-dim mr-1">Was this helpful?</span>
        {feedbackSent ? (
          <span className="text-2xs text-muted-foreground">
            {feedbackSent === "helpful" ? "Thanks for the feedback!" : "Got it, we'll improve."}
          </span>
        ) : (
          <>
            <button
              type="button"
              onClick={() => handleFeedback("helpful")}
              disabled={loading}
              className="flex items-center gap-1 px-2 py-1 rounded text-2xs text-muted-foreground hover:text-success hover:bg-success/10 transition-colors disabled:opacity-50"
            >
              <ThumbsUp className="size-3" />
              Helpful
            </button>
            <button
              type="button"
              onClick={() => handleFeedback("not_helpful")}
              disabled={loading}
              className="flex items-center gap-1 px-2 py-1 rounded text-2xs text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-50"
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
