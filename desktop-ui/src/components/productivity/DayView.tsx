import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { TimelineBar } from './Timeline';
import { PomodoroTimer } from './PomodoroTimer';
import { FocusSessionsList } from './FocusSessionsList';
import { ActivityFeed } from './ActivityFeed';
import { WorkHoursCard } from './WorkHoursCard';
import { ProductivityScoreRing } from './ProductivityScoreRing';
import { BreakdownDonuts } from './BreakdownDonuts';
import { CategoriesList } from './CategoriesList';
import { GoalsProgress } from './GoalsProgress';
import { AiSummaryCard } from './AiSummaryCard';
import { TopApps } from './TopApps';
import type { ProductivitySummary } from '../../lib/types';

interface DayViewProps {
  date: string;
}

export function DayView({ date }: DayViewProps) {
  const { data: summary, refetch } = useQuery<ProductivitySummary | null>('productivity_today', undefined, null);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    const k = payload?.entityKind;
    if (k === 'productivity' || k === 'focus_session') refetch();
  });

  const breakdownSegments = summary ? [
    { name: 'Focus', value: summary.totalFocusSecs, color: 'var(--brand)' },
    { name: 'Active', value: summary.totalActiveSecs - summary.totalFocusSecs - summary.totalBreakSecs, color: 'var(--purple)' },
    { name: 'Breaks', value: summary.totalBreakSecs, color: 'var(--info)' },
  ] : [];

  return (
    <div className="grid grid-cols-3 gap-4 auto-rows-min">
      {/* Row 1: Timeline (full width) */}
      <TimelineBar date={date} />

      {/* Row 2-3: Left column */}
      <div className="flex flex-col gap-4">
        <PomodoroTimer />
        <ActivityFeed />
      </div>

      {/* Row 2-3: Center column */}
      <div className="flex flex-col gap-4">
        <FocusSessionsList date={date} />
        <TopApps apps={summary?.topApps ?? []} />
      </div>

      {/* Row 2-3: Right column */}
      <div className="flex flex-col gap-4">
        <WorkHoursCard totalActiveSecs={summary?.totalActiveSecs ?? 0} />
        <div className="bg-surface-base rounded-xl p-4 flex items-center justify-center relative">
          <ProductivityScoreRing score={summary?.productivityScore ?? 0} />
        </div>
        <BreakdownDonuts segments={breakdownSegments} totalSecs={summary?.totalActiveSecs ?? 0} />
        <CategoriesList categories={summary?.topCategories ?? []} totalSecs={summary?.totalActiveSecs ?? 0} />
        <AiSummaryCard summary={summary?.aiSummary ?? null} />
      </div>

      {/* Row 4: Goals (spans left+center) */}
      <div className="col-span-2">
        <GoalsProgress />
      </div>
    </div>
  );
}
