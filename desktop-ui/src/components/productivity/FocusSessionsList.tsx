import { useQuery } from '../../hooks/useQuery';
import { formatTime } from '../../lib/dates';
import type { FocusSession } from '../../lib/types';

interface FocusSessionsListProps {
  date: string;
}

function qualityBadge(score: number | null): { text: string; color: string } {
  if (score == null) return { text: '—', color: 'text-dim' };
  const pct = Math.round(score * 100);
  if (pct >= 80) return { text: `${pct}%`, color: 'text-success' };
  if (pct >= 50) return { text: `${pct}%`, color: 'text-brand' };
  return { text: `${pct}%`, color: 'text-destructive' };
}

export function FocusSessionsList({ date }: FocusSessionsListProps) {
  const { data: sessions } = useQuery<FocusSession[]>('productivity_sessions', { date }, []);

  if (sessions.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Focus Sessions</h2>
        <p className="text-[12px] font-light text-dim">No sessions today</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Focus Sessions</h2>
      <div className="flex flex-col gap-1.5">
        {sessions.map(s => {
          const quality = qualityBadge(s.qualityScore);
          return (
            <div
              key={s.id}
              className="flex items-center justify-between py-1.5 border-b border-border-subtle last:border-b-0"
            >
              <div className="flex items-center gap-2 text-[11px] font-light">
                <span className="text-muted tabular-nums">{formatTime(s.startedAt)}</span>
                <span className="text-primary">
                  {s.actualMins != null ? `${s.actualMins}m` : 'In progress'}
                </span>
                {s.interruptions > 0 && (
                  <span className="text-dim">{s.interruptions} int.</span>
                )}
              </div>
              <span className={`text-[11px] font-light tabular-nums ${quality.color}`}>
                {quality.text}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
