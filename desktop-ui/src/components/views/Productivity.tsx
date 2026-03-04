import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { Sidebar } from '../layout/Sidebar';
import { FocusStatusCard } from '../productivity/FocusStatusCard';
import { TodaySummary } from '../productivity/TodaySummary';
import { Timeline } from '../productivity/Timeline';
import { TopApps } from '../productivity/TopApps';
import { WeeklyTrend } from '../productivity/WeeklyTrend';
import { FocusSessionsList } from '../productivity/FocusSessionsList';
import type { ProductivitySummary } from '../../lib/types';

export function Productivity() {
  const today = new Date().toISOString().slice(0, 10);

  const { data: summary, refetch } = useQuery<ProductivitySummary | null>('productivity_today', undefined, null);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    const k = payload?.entityKind;
    if (k === 'productivity' || k === 'focus_session') refetch();
  });

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar active="Productivity" />
      <div className="flex-1 flex flex-col overflow-hidden">
        <div className="h-14 bg-background flex items-center px-4">
          <h1 className="text-lg font-semibold text-primary">Productivity</h1>
        </div>
        <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
          <FocusStatusCard />
          <TodaySummary summary={summary} />
          <Timeline date={today} />
          <div className="grid grid-cols-2 gap-4">
            <FocusSessionsList date={today} />
            <TopApps apps={summary?.topApps ?? []} />
          </div>
          <WeeklyTrend />
        </div>
      </div>
    </div>
  );
}
