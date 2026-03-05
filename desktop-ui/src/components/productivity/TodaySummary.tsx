import { formatHumanDuration } from '../../lib/dates';
import type { ProductivitySummary } from '../../lib/types';

interface TodaySummaryProps {
  summary: ProductivitySummary | null;
}

function BreakdownBar({ productive, neutral, distracting }: { productive: number; neutral: number; distracting: number }) {
  const total = productive + neutral + distracting;
  if (total === 0) return <div className="h-2 rounded-full bg-surface-raised" />;

  const pPct = (productive / total) * 100;
  const nPct = (neutral / total) * 100;
  const dPct = (distracting / total) * 100;

  return (
    <div className="flex h-2 rounded-full overflow-hidden gap-px">
      {pPct > 0 && <div className="bg-success rounded-full" style={{ width: `${pPct}%` }} />}
      {nPct > 0 && <div className="bg-text-muted rounded-full" style={{ width: `${nPct}%` }} />}
      {dPct > 0 && <div className="bg-destructive rounded-full" style={{ width: `${dPct}%` }} />}
    </div>
  );
}

interface StatCardProps {
  label: string;
  value: string;
  color?: string;
}

function StatCard({ label, value, color }: StatCardProps) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[11px] font-light text-muted">{label}</span>
      <span className={`text-[18px] font-medium ${color ?? 'text-primary'}`}>{value}</span>
    </div>
  );
}

export function TodaySummary({ summary }: TodaySummaryProps) {

  if (!summary) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Today&apos;s Summary</h2>
        <p className="text-[12px] font-light text-dim">No activity recorded today</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-4">
      <h2 className="text-[13px] font-medium text-secondary">Today&apos;s Summary</h2>

      <div className="grid grid-cols-4 gap-4">
        <StatCard label="Active Time" value={formatHumanDuration(summary.totalActiveSecs)} />
        <StatCard label="Focus Time" value={formatHumanDuration(summary.totalFocusSecs)} color="text-brand" />
        <StatCard label="Break Time" value={formatHumanDuration(summary.totalBreakSecs)} />
        <StatCard label="Idle Time" value={formatHumanDuration(summary.totalIdleSecs)} color="text-dim" />
      </div>

      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between text-[11px] font-light">
          <span className="text-success">Productive {formatHumanDuration(summary.productiveSecs)}</span>
          <span className="text-muted">Neutral {formatHumanDuration(summary.neutralSecs)}</span>
          <span className="text-destructive">Distracting {formatHumanDuration(summary.distractingSecs)}</span>
        </div>
        <BreakdownBar
          productive={summary.productiveSecs}
          neutral={summary.neutralSecs}
          distracting={summary.distractingSecs}
        />
      </div>

      <div className="grid grid-cols-3 gap-4 pt-1 border-t border-border-subtle">
        <StatCard label="Focus Sessions" value={String(summary.focusSessionsCount)} />
        <StatCard label="Interruptions" value={String(summary.interruptionsCount)} />
        <StatCard label="Context Switches" value={String(summary.contextSwitches)} />
      </div>
    </div>
  );
}
