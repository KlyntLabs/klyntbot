import { useQuery } from '../../hooks/useQuery';
import { formatHumanDuration, formatDayLabel } from '../../lib/dates';
import type { ProductivitySummary } from '../../lib/types';

export function WeeklyTrend() {
  const { data: summaries } = useQuery<ProductivitySummary[]>('productivity_weekly', undefined, []);

  if (summaries.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Weekly Trend</h2>
        <p className="text-[12px] font-light text-dim">No weekly data yet</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Weekly Trend</h2>

      <div className="grid grid-cols-[60px_1fr_1fr_1fr_80px] gap-x-3 gap-y-1.5 text-[11px] font-light">
        {/* Header */}
        <span className="text-dim">Day</span>
        <span className="text-dim">Focus</span>
        <span className="text-dim">Active</span>
        <span className="text-dim">Productive</span>
        <span className="text-dim text-right">Switches</span>

        {summaries.map(s => (
          <Row key={s.date} summary={s} />
        ))}
      </div>
    </div>
  );
}

function Row({ summary: s }: { summary: ProductivitySummary }) {
  return (
    <>
      <span className="text-secondary">{formatDayLabel(s.date)}</span>
      <span className="text-brand tabular-nums">{formatHumanDuration(s.totalFocusSecs)}</span>
      <span className="text-primary tabular-nums">{formatHumanDuration(s.totalActiveSecs)}</span>
      <span className="text-success tabular-nums">{formatHumanDuration(s.productiveSecs)}</span>
      <span className="text-muted tabular-nums text-right">{s.contextSwitches}</span>
    </>
  );
}
