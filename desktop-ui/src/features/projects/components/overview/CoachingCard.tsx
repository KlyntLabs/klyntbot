import { useCoachingNudge } from "@shared/hooks/useCoachingNudge";
import { Brain, ThumbsUp, X } from "lucide-react";
import { useNavigate } from "react-router";

export function CoachingCard() {
  const navigate = useNavigate();
  const { nudge, handleFeedback } = useCoachingNudge({ autoCollapseMs: 60_000 });

  return (
    <button
      type="button"
      className="glass-card rounded-xl p-5 cursor-pointer hover:bg-control-hover/5 transition-colors w-full text-left"
      onClick={() => navigate("/coaching")}
    >
      <p className="text-ui-xs text-fg-secondary uppercase tracking-wider mb-3">Coaching</p>

      {nudge ? (
        <div className="flex flex-col gap-2">
          <p className="text-ui-sm text-fg leading-relaxed">{nudge.message}</p>
          <div className="flex items-center gap-2 mt-1">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleFeedback(nudge.id, "helpful");
              }}
              className="flex items-center gap-1 text-ui-xs text-fg-secondary hover:text-status-success transition-colors"
            >
              <ThumbsUp className="size-3" />
              Helpful
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleFeedback(nudge.id, "dismissed");
              }}
              className="flex items-center gap-1 text-ui-xs text-fg-secondary hover:text-status-danger transition-colors"
            >
              <X className="size-3" />
              Dismiss
            </button>
          </div>
        </div>
      ) : (
        <div className="flex items-center gap-2">
          <Brain className="size-4 text-fg-secondary/50" />
          <p className="text-ui-xs text-fg-secondary">
            No active coaching — Deep work mode detected
          </p>
        </div>
      )}
    </button>
  );
}
