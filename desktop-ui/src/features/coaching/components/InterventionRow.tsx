import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { formatTime } from "@shared/lib/dates";
import { ThumbsUp, X } from "lucide-react";
import { useNavigate } from "react-router";
import { FeedbackBadge } from "./FeedbackBadge";

interface InterventionRowProps {
  id: string;
  message: string;
  interventionType: string;
  triggerName: string;
  feedback: string | null;
  deliveredAt: string;
  actionUrl?: string;
}

export function InterventionRow({
  id,
  message,
  interventionType,
  triggerName,
  feedback,
  deliveredAt,
  actionUrl,
}: InterventionRowProps) {
  const navigate = useNavigate();
  const { mutate: submitFeedback } = useMutation("coaching_submit_feedback");

  const handleFeedback = async (response: string) => {
    await submitFeedback({ intervention_id: id, response });
    invalidateQueries("coaching_intervention_log");
    invalidateQueries("coaching_feedback_stats");
  };

  const canGiveFeedback = !feedback || feedback === "ignored";

  return (
    <div className="flex items-start gap-3 py-3 border-b border-border last:border-0">
      <span className="text-2xs text-dim tabular-nums w-14 pt-0.5 shrink-0">
        {formatTime(deliveredAt)}
      </span>

      <div className="flex-1 min-w-0">
        <p className="text-[11px] text-foreground leading-relaxed">{message}</p>
        <div className="flex items-center gap-2 mt-1.5">
          <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-accent/30 text-dim">
            {interventionType}
          </span>
          <span className="text-[9px] text-dim">{triggerName}</span>
        </div>
      </div>

      {actionUrl && (
        <button
          type="button"
          onClick={() => navigate(actionUrl)}
          className="text-2xs text-brand hover:underline shrink-0"
        >
          Open →
        </button>
      )}

      <div className="flex items-center gap-2 shrink-0">
        <FeedbackBadge feedback={feedback} />
        {canGiveFeedback && (
          <>
            <button
              type="button"
              onClick={() => handleFeedback("helpful")}
              className="flex items-center gap-1 text-2xs text-muted-foreground hover:text-success transition-colors"
              title="Mark as helpful"
            >
              <ThumbsUp className="size-3" />
            </button>
            <button
              type="button"
              onClick={() => handleFeedback("dismissed")}
              className="flex items-center gap-1 text-2xs text-muted-foreground hover:text-destructive transition-colors"
              title="Dismiss"
            >
              <X className="size-3" />
            </button>
          </>
        )}
      </div>
    </div>
  );
}
