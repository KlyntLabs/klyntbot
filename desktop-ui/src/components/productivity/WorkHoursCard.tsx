import { formatLongDuration } from '../../lib/dates';

interface WorkHoursCardProps {
  totalActiveSecs: number;
  workDayHours?: number;
}

export function WorkHoursCard({ totalActiveSecs, workDayHours = 8 }: WorkHoursCardProps) {
  const pct = Math.min((totalActiveSecs / (workDayHours * 3600)) * 100, 100);

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-2">
      <h2 className="text-[13px] font-medium text-secondary">Work Hours</h2>
      <div className="flex items-baseline justify-between">
        <span className="text-[28px] font-light text-primary tabular-nums">
          {formatLongDuration(totalActiveSecs)}
        </span>
        <div className="flex flex-col items-end">
          <span className="text-[11px] font-light text-dim">Percent of work day</span>
          <span className="text-[18px] font-light text-primary tabular-nums">{pct.toFixed(1)}%</span>
          <span className="text-[10px] font-light text-dim">of {formatLongDuration(workDayHours * 3600)}</span>
        </div>
      </div>
    </div>
  );
}
