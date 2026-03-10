import { useMemo } from "react";
import { useQuery } from "@shared/hooks/useQuery";
import { shiftDate } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { BreakdownDonuts } from "./BreakdownDonuts";
import { CategoriesList } from "./CategoriesList";
import { GoalsProgress } from "./GoalsProgress";
import { buildBreakdownSegments } from "../lib/constants";
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

    const allCats = new Map<string, { categoryId: string; category: string; categoryType: "productive" | "neutral" | "distracting"; durationSecs: number }>();
    for (const s of summaries) {
      for (const c of s.topCategories) {
        const key = c.category;
        const existing = allCats.get(key);
        if (existing) {
          existing.durationSecs += c.durationSecs;
        } else {
          allCats.set(key, { ...c });
        }
      }
    }
    const cats = Array.from(allCats.values())
      .sort((a, b) => b.durationSecs - a.durationSecs);

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
