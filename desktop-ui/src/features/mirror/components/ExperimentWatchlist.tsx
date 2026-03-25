import { useMutation } from "@shared/hooks/useMutation";
import { FlaskConical, Play, X } from "lucide-react";

interface TrialPreview {
  id: string;
  trialId: string;
  startedAt: string;
  previewAt: string;
  messagesScored: number;
  earlySignals: {
    correctionRateDelta: number;
    confidenceTrend: string;
    dominantSkillShift: string | null;
  };
  recommendation: string;
  narrative: string;
}

interface ExperimentWatchlistProps {
  previews: TrialPreview[];
  onAction?: () => void;
}

export function ExperimentWatchlist({ previews, onAction }: ExperimentWatchlistProps) {
  const { mutate: kill } = useMutation<void, { trialId: string }>("kill_trial");
  const { mutate: cont } = useMutation<void, { trialId: string }>("continue_trial");

  if (previews.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-1.5">
        <FlaskConical className="size-3.5" />
        Experiment Watchlist
      </h2>

      {previews.map((preview) => {
        const isKill = preview.recommendation === "Kill";
        const isContinue = preview.recommendation === "Continue";

        return (
          <div
            key={preview.id}
            className={`glass-panel rounded-xl p-4 ${isKill ? "border border-destructive/30" : ""}`}
          >
            <div className="flex items-center justify-between mb-1">
              <span className="text-[12px] font-medium text-foreground">
                Trial {preview.trialId.slice(0, 8)}
              </span>
              <span
                className={`text-2xs px-1.5 py-0.5 rounded ${
                  isKill
                    ? "text-destructive bg-destructive/10"
                    : isContinue
                      ? "text-success bg-success/10"
                      : "text-muted-foreground bg-muted/10"
                }`}
              >
                {preview.recommendation}
              </span>
            </div>

            <p className="text-[11px] text-muted-foreground">{preview.narrative}</p>

            <div className="flex items-center gap-2 mt-3">
              <button
                type="button"
                onClick={async () => {
                  await kill({ trialId: preview.trialId });
                  onAction?.();
                }}
                className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-destructive bg-destructive/10 hover:bg-destructive/20 transition-colors"
              >
                <X className="size-3" />
                Kill it
              </button>
              <button
                type="button"
                onClick={async () => {
                  await cont({ trialId: preview.trialId });
                  onAction?.();
                }}
                className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-success bg-success/10 hover:bg-success/20 transition-colors"
              >
                <Play className="size-3" />
                Let it run
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
