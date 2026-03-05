import { formatLongDuration } from "../../lib/dates";

interface WorkHoursCardProps {
  totalActiveSecs: number;
  workDayHours?: number;
}

export function WorkHoursCard({ totalActiveSecs, workDayHours = 8 }: WorkHoursCardProps) {
  const targetSecs = workDayHours * 3600;
  const pct = Math.min((totalActiveSecs / targetSecs) * 100, 100);
  const remaining = Math.max(targetSecs - totalActiveSecs, 0);
  const isComplete = pct >= 100;

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Work Hours</h2>
        <span className="text-[10px] font-light text-dim">of {workDayHours}h target</span>
      </div>

      {/* Hero number */}
      <div className="flex items-baseline gap-2">
        <span className="text-[28px] font-light text-primary tabular-nums leading-none">
          {formatLongDuration(totalActiveSecs)}
        </span>
        <span
          className="text-[13px] font-medium tabular-nums"
          style={{ color: isComplete ? "var(--success)" : "var(--brand)" }}
        >
          {pct.toFixed(0)}%
        </span>
      </div>

      {/* Progress bar */}
      <div className="relative">
        <div className="h-2 rounded-full bg-surface-raised overflow-hidden">
          <div
            className="h-full rounded-full transition-all duration-700"
            style={{
              width: `${pct}%`,
              background: isComplete
                ? "linear-gradient(90deg, var(--success), var(--success)cc)"
                : "linear-gradient(90deg, var(--brand), var(--brand)cc)",
              boxShadow: isComplete ? "0 0 12px var(--success)44" : "0 0 12px var(--brand)33",
            }}
          />
        </div>
      </div>

      {/* Remaining */}
      <span className="text-[10px] font-light text-dim">
        {isComplete ? "Target reached" : `${formatLongDuration(remaining)} remaining`}
      </span>
    </div>
  );
}
