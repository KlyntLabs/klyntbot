import { Progress } from '../ui/Progress';

interface SubtaskProgressProps {
  total: number;
  completed: number;
}

export function SubtaskProgress({ total, completed }: SubtaskProgressProps) {
  if (total === 0) return null;

  const pct = Math.round((completed / total) * 100);

  return (
    <div className="inline-flex items-center gap-1.5 ml-2 flex-shrink-0">
      <Progress value={pct} className="w-10 h-1" />
      <span className="text-[11px] text-muted font-light tabular-nums">
        {completed}/{total}
      </span>
    </div>
  );
}
