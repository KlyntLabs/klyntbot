import { useMemo } from 'react';
import { useQuery } from '../../hooks/useQuery';
import type { ActivityTimeline, ActivityCategory } from '../../lib/types';

interface TimelineBarProps {
  date: string;
}

interface Block {
  leftPct: number;
  widthPct: number;
  color: string;
  label: string;
}

const START_HOUR = 6;
const END_HOUR = 22;
const SPAN_HOURS = END_HOUR - START_HOUR;

// Heuristic app classification when no category is assigned
const PRODUCTIVE_APPS = new Set([
  'code', 'visual studio code', 'cursor', 'xcode', 'intellij', 'webstorm',
  'terminal', 'iterm2', 'warp', 'alacritty', 'kitty', 'ghostty',
  'figma', 'sketch', 'linear', 'notion', 'obsidian', 'sublime text',
  'github desktop', 'tower', 'postman', 'docker', 'tableplus',
]);
const DISTRACTING_APPS = new Set([
  'twitter', 'x', 'facebook', 'instagram', 'tiktok', 'reddit',
  'youtube', 'netflix', 'twitch', 'discord',
]);

function resolveColor(categoryType: string | undefined, isIdle: boolean, appName: string): string {
  if (isIdle) return 'var(--surface-lowest)';
  if (categoryType === 'productive') return 'var(--success)';
  if (categoryType === 'distracting') return 'var(--destructive)';
  if (categoryType === 'neutral') return 'var(--text-muted)';

  // Fallback: heuristic from app name
  const lower = appName.toLowerCase();
  if (PRODUCTIVE_APPS.has(lower)) return 'var(--success)';
  if (DISTRACTING_APPS.has(lower)) return 'var(--destructive)';

  // Uncategorized non-idle activity gets a softer brand color
  return 'var(--brand)';
}

const TICK_LABELS: { hour: number; label: string }[] = [];
for (let h = START_HOUR; h <= END_HOUR; h += 2) {
  TICK_LABELS.push({
    hour: h,
    label: h === 0 ? '12a' : h < 12 ? `${h}a` : h === 12 ? '12p' : `${h - 12}p`,
  });
}

export function TimelineBar({ date }: TimelineBarProps) {
  const { data: events } = useQuery<ActivityTimeline[]>('productivity_timeline', { date }, []);
  const { data: categories } = useQuery<ActivityCategory[]>('productivity_categories', undefined, []);

  const categoryMap = useMemo(
    () => new Map(categories.map((c) => [c.id, c])),
    [categories],
  );

  const blocks: Block[] = useMemo(() => {
    if (events.length === 0) return [];
    const spanSecs = SPAN_HOURS * 3600;
    const startSecs = START_HOUR * 3600;

    return events
      .map((e) => {
        const start = new Date(e.startedAt);
        const eSecs = start.getHours() * 3600 + start.getMinutes() * 60 + start.getSeconds();
        const dur = e.durationSecs ?? 0;
        if (eSecs + dur < startSecs || eSecs > END_HOUR * 3600) return null;

        const clampedStart = Math.max(eSecs - startSecs, 0);
        const clampedEnd = Math.min(eSecs + dur - startSecs, spanSecs);
        const cat = e.categoryId ? categoryMap.get(e.categoryId) : undefined;

        return {
          leftPct: (clampedStart / spanSecs) * 100,
          widthPct: Math.max(((clampedEnd - clampedStart) / spanSecs) * 100, 0.3),
          color: resolveColor(cat?.categoryType, e.isIdle, e.appName),
          label: e.appName,
        };
      })
      .filter(Boolean) as Block[];
  }, [events, categoryMap]);

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-2 col-span-3">
      <h2 className="text-[13px] font-medium text-secondary">Timeline</h2>

      <div className="relative h-10 rounded-lg bg-surface-raised overflow-hidden">
        {blocks.map((b, i) => (
          <div
            key={i}
            className="absolute top-0 h-full rounded-sm"
            style={{
              left: `${b.leftPct}%`,
              width: `${b.widthPct}%`,
              backgroundColor: b.color,
            }}
            title={b.label}
          />
        ))}
      </div>

      <div className="flex justify-between text-[9px] font-light text-dim px-0.5">
        {TICK_LABELS.map(({ hour, label }) => (
          <span key={hour} style={{ width: `${100 / TICK_LABELS.length}%` }}>{label}</span>
        ))}
      </div>

      <div className="flex items-center gap-4 text-[10px] font-light text-muted">
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-success" />Productive</span>
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-brand" />Uncategorized</span>
        <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-destructive" />Distracting</span>
      </div>
    </div>
  );
}
