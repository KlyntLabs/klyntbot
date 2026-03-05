import { useMemo } from "react";
import { useQuery } from "../../hooks/useQuery";
import { shiftDate } from "../../lib/dates";
import type { ProductivitySummary } from "../../lib/types";
import { BreakdownDonuts } from "./BreakdownDonuts";
import { CategoriesList } from "./CategoriesList";
import { GoalsProgress } from "./GoalsProgress";
import { buildBreakdownSegments } from "./shared";
import { TopApps } from "./TopApps";
import { WeeklyChart } from "./WeeklyChart";
import { WeeklyStats } from "./WeeklyStats";

interface WeekViewProps {
  weekStart: string;
}

export function WeekView({ weekStart }: WeekViewProps) {
  const weekEnd = useMemo(() => shiftDate(weekStart, 6), [weekStart]);

  const { data: summaries } = useQuery<ProductivitySummary[]>(
    "productivity_summary_range",
    { start_date: weekStart, end_date: weekEnd },
    [],
  );

  const { totalActive, totalFocus, totalBreak, topApps, topCats } = useMemo(() => {
    const active = summaries.reduce((s, d) => s + d.totalActiveSecs, 0);
    const focus = summaries.reduce((s, d) => s + d.totalFocusSecs, 0);
    const brk = summaries.reduce((s, d) => s + d.totalBreakSecs, 0);

    const allApps = new Map<string, number>();
    for (const s of summaries) {
      for (const a of s.topApps) {
        allApps.set(a.appName, (allApps.get(a.appName) ?? 0) + a.durationSecs);
      }
    }
    const apps = Array.from(allApps.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 10)
      .map(([appName, durationSecs]) => ({ appName, durationSecs, category: null }));

    const allCats = new Map<string, number>();
    for (const s of summaries) {
      for (const c of s.topCategories) {
        allCats.set(c.category, (allCats.get(c.category) ?? 0) + c.durationSecs);
      }
    }
    const cats = Array.from(allCats.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([category, durationSecs]) => ({ category, durationSecs }));

    return {
      totalActive: active,
      totalFocus: focus,
      totalBreak: brk,
      topApps: apps,
      topCats: cats,
    };
  }, [summaries]);

  const breakdownSegments = useMemo(
    () => buildBreakdownSegments(totalActive, totalFocus, totalBreak),
    [totalActive, totalFocus, totalBreak],
  );

  return (
    <div className="grid grid-cols-3 gap-4 auto-rows-min">
      <WeeklyChart summaries={summaries} />
      <WeeklyStats summaries={summaries} />
      <BreakdownDonuts segments={breakdownSegments} totalSecs={totalActive} />
      <CategoriesList categories={topCats} totalSecs={totalActive} />
      <TopApps apps={topApps} />
      <div className="col-span-2">
        <GoalsProgress />
      </div>
    </div>
  );
}
