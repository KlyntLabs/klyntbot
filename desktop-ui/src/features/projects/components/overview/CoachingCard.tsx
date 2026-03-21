import { useCoachingNudge } from "@shared/hooks/useCoachingNudge";
import { Brain, ThumbsUp, X } from "lucide-react";
import { useNavigate } from "react-router";

export function CoachingCard() {
  const navigate = useNavigate();
  const { nudge, handleFeedback } = useCoachingNudge({ autoCollapseMs: 60_000 });

  return (
    <div
      className="glass-card rounded-xl p-5 cursor-pointer hover:bg-accent/5 transition-colors"
      onClick={() => navigate("/coaching")}
      onKeyDown={(e) => e.key === "Enter" && navigate("/coaching")}
      role="button"
      tabIndex={0}
    >
      <p className="text-[10px] text-muted-foreground uppercase tracking-wider mb-3">Coaching</p>

      {nudge ? (
        <div className="flex flex-col gap-2">
          <p className="text-xs text-foreground leading-relaxed">{nudge.message}</p>
          <div className="flex items-center gap-2 mt-1">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleFeedback(nudge.id, "helpful");
              }}
              className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-success transition-colors"
            >
              <ThumbsUp className="w-3 h-3" />
              Helpful
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleFeedback(nudge.id, "dismissed");
              }}
              className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-destructive transition-colors"
            >
              <X className="w-3 h-3" />
              Dismiss
            </button>
          </div>
        </div>
      ) : (
        <div className="flex items-center gap-2">
          <Brain className="w-4 h-4 text-muted-foreground/50" />
          <p className="text-[11px] text-muted-foreground">
            No active coaching — Deep work mode detected
          </p>
        </div>
      )}
    </div>
  );
}
