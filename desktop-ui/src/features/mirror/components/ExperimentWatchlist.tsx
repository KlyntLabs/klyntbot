import { useMutation } from "@shared/hooks/useMutation";
import { FlaskConical, Play, Square } from "lucide-react";

export interface TrialPreview {
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

function formatDelta(delta: number): string {
  const pct = (delta * 100).toFixed(1);
  return delta >= 0 ? `+${pct}%` : `${pct}%`;
}

export function ExperimentWatchlist({ previews, onAction }: ExperimentWatchlistProps) {
  const { mutate: kill } = useMutation<void, { trialId: string }>("kill_trial");
  const { mutate: cont } = useMutation<void, { trialId: string }>("continue_trial");

  if (previews.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-ui font-medium text-fg-secondary flex items-center gap-1.5">
        <FlaskConical className="size-3.5" />
        Experiment Watchlist
      </h2>

      {previews.map((preview) => {
        const isKill = preview.recommendation === "Kill";
        const isContinue = preview.recommendation === "Continue";
        const delta = preview.earlySignals.correctionRateDelta;

        return (
          <div key={preview.id} className="island rounded-xl p-4">
            <div className="flex items-center justify-between">
              <span className="text-ui-sm font-medium text-fg">
                Trial {preview.trialId.slice(0, 8)}
              </span>
              <span className="text-ui-xs text-fg-dim">
                {preview.messagesScored} messages scored
              </span>
            </div>

            <p className="text-ui-xs text-fg-secondary mt-1 leading-relaxed">{preview.narrative}</p>

            <div className="flex items-center justify-between mt-3 pt-3 border-t border-separator">
              <span
                className={`text-ui-xs font-medium ${
                  isKill
                    ? "text-status-danger"
                    : isContinue
                      ? "text-status-success"
                      : "text-fg-secondary"
                }`}
              >
                {isKill ? "Recommend kill" : isContinue ? "Looking good" : "Needs more data"}
                {" \u00B7 "}
                <span className={delta < 0 ? "text-status-danger" : "text-status-success"}>
                  {formatDelta(delta)}
                </span>
              </span>

              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={async () => {
                    await kill({ trialId: preview.trialId });
                    onAction?.();
                  }}
                  className="flex items-center gap-1 px-2 py-1 rounded text-ui-xs text-fg-secondary hover:text-status-danger hover:bg-status-danger/10 transition-colors"
                >
                  <Square className="size-3" />
                  Kill
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    await cont({ trialId: preview.trialId });
                    onAction?.();
                  }}
                  className="flex items-center gap-1 px-2 py-1 rounded text-ui-xs text-fg-secondary hover:text-status-success hover:bg-status-success/10 transition-colors"
                >
                  <Play className="size-3" />
                  Continue
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
