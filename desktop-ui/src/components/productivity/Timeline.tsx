import { useMemo } from 'react';
import { useQuery } from '../../hooks/useQuery';
import type { ActivityTimeline, ActivityCategory } from '../../lib/types';

interface TimelineProps {
  date: string;
}

interface Block {
  startHour: number;
  widthPct: number;
  color: string;
  label: string;
}

const TICK_HOURS = [
  { hour: 0, label: '12a' },
  { hour: 6, label: '6a' },
  { hour: 12, label: '12p' },
  { hour: 18, label: '6p' },
];

function categoryColor(categoryType: string | undefined, isIdle: boolean): string {
  if (isIdle) return 'bg-surface-lowest';
  switch (categoryType) {
    case 'productive': return 'bg-success';
    case 'distracting': return 'bg-destructive';
    default: return 'bg-text-muted';
  }
}

export function Timeline({ date }: TimelineProps) {
  const { data: events } = useQuery<ActivityTimeline[]>('productivity_timeline', { date }, []);
  const { data: categories } = useQuery<ActivityCategory[]>('productivity_categories', undefined, []);

  const categoryMap = useMemo(
    () => new Map(categories.map(c => [c.id, c])),
    [categories],
  );

  const blocks: Block[] = useMemo(() => {
    if (events.length === 0) return [];

    const totalSecs = 24 * 3600;
    return events.map(e => {
      const start = new Date(e.startedAt);
      const startSecs = start.getHours() * 3600 + start.getMinutes() * 60 + start.getSeconds();
      const cat = e.categoryId ? categoryMap.get(e.categoryId) : undefined;

      return {
        startHour: startSecs / 3600,
        widthPct: ((e.durationSecs ?? 0) / totalSecs) * 100,
        color: categoryColor(cat?.categoryType, e.isIdle),
        label: e.appName,
      };
    });
  }, [events, categoryMap]);

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Activity Timeline</h2>

      {/* Timeline bar */}
      <div className="relative h-6 rounded-md bg-surface-raised overflow-hidden">
        {blocks.map((b, i) => (
          <div
            key={i}
            className={`absolute top-0 h-full ${b.color}`}
            style={{ left: `${(b.startHour / 24) * 100}%`, width: `${Math.max(b.widthPct, 0.2)}%` }}
            title={b.label}
          />
        ))}
      </div>

      {/* Hour labels */}
      <div className="flex justify-between text-[9px] font-light text-dim px-0.5">
        {TICK_HOURS.map(({ hour, label }) => (
          <span key={hour}>{label}</span>
        ))}
        <span>12a</span>
      </div>

      {/* Legend */}
      <div className="flex items-center gap-4 text-[10px] font-light text-muted">
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-success" />Productive</span>
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-text-muted" />Neutral</span>
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-destructive" />Distracting</span>
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-surface-lowest" />Idle</span>
      </div>
    </div>
  );
}
