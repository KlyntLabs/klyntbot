import { X } from "lucide-react";

interface SessionProgressProps {
  remaining: number;
  total: number;
  avgScore: number | null;
  onExit: () => void;
}

export function SessionProgress({ remaining, total, avgScore, onExit }: SessionProgressProps) {
  const reviewed = total - remaining;
  const progressPct = total > 0 ? (reviewed / total) * 100 : 0;
  const scoreDisplay = avgScore != null ? `${Math.round(avgScore * 100)}%` : null;

  return (
    <div className="flex flex-col gap-1">
      {/* Thin progress bar */}
      <div className="h-0.5 rounded-full bg-white/[0.06] overflow-hidden">
        <div
          className="h-full bg-accent/60 rounded-full transition-[width] duration-300 origin-left"
          style={{ width: `${progressPct}%` }}
        />
      </div>

      {/* Stats row */}
      <div className="flex items-center gap-2">
        <span className="text-[9px] text-dim flex-1">
          {remaining} remaining
          {scoreDisplay && (
            <span className="ml-1.5 text-muted-foreground">· avg {scoreDisplay}</span>
          )}
        </span>
        <button
          type="button"
          onClick={onExit}
          className="p-0.5 text-dim hover:text-foreground"
          aria-label="Exit review"
        >
          <X size={10} />
        </button>
      </div>
    </div>
  );
}
