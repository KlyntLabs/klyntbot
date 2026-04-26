import { CheckCircle, Clock, Target } from "lucide-react";

interface PostSessionProps {
  totalReviewed: number;
  correctCount: number;
  elapsedSeconds: number;
  onBackToDashboard: () => void;
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  if (m === 0) return `${s}s`;
  return `${m}m ${s}s`;
}

export function PostSession({
  totalReviewed,
  correctCount,
  elapsedSeconds,
  onBackToDashboard,
}: PostSessionProps) {
  const accuracy = totalReviewed > 0 ? Math.round((correctCount / totalReviewed) * 100) : 0;

  return (
    <div className="flex-1 flex items-center justify-center">
      <div className="text-center max-w-sm w-full space-y-6 animate-[fade-in-up_0.3s_ease-out]">
        <div>
          <CheckCircle size={40} className="mx-auto text-emerald-400 mb-3" strokeWidth={1.5} />
          <h2 className="text-xl font-semibold text-foreground">Review Complete!</h2>
        </div>

        <div className="flex gap-3 justify-center">
          <div className="glass-card px-4 py-3 text-center flex-1">
            <p className="text-2xl font-semibold text-foreground tabular-nums">{totalReviewed}</p>
            <p className="text-[11px] text-muted-foreground mt-0.5">Cards reviewed</p>
          </div>

          <div className="glass-card px-4 py-3 text-center flex-1">
            <div className="flex items-center justify-center gap-1">
              <Target size={14} className="text-brand" strokeWidth={1.5} />
              <p className="text-2xl font-semibold text-foreground tabular-nums">{accuracy}%</p>
            </div>
            <p className="text-[11px] text-muted-foreground mt-0.5">Accuracy</p>
          </div>

          <div className="glass-card px-4 py-3 text-center flex-1">
            <div className="flex items-center justify-center gap-1">
              <Clock size={14} className="text-info" strokeWidth={1.5} />
              <p className="text-2xl font-semibold text-foreground tabular-nums">
                {formatTime(elapsedSeconds)}
              </p>
            </div>
            <p className="text-[11px] text-muted-foreground mt-0.5">Time spent</p>
          </div>
        </div>

        <button
          type="button"
          onClick={onBackToDashboard}
          className="glass-button px-5 py-2.5 text-sm text-foreground"
        >
          Back to Dashboard
        </button>
      </div>
    </div>
  );
}
