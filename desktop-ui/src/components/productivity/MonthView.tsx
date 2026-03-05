import { useMemo } from "react";
import { useQuery } from "../../hooks/useQuery";
import { monthEndISO } from "../../lib/dates";
import type { ProductivitySummary } from "../../lib/types";
import { BreakdownDonuts } from "./BreakdownDonuts";
import { CategoriesList } from "./CategoriesList";
import { MonthlyChart } from "./MonthlyChart";
import { MonthlyStats } from "./MonthlyStats";
import { buildBreakdownSegments } from "./shared";

interface MonthViewProps {
  yearMonth: string;
}

export function MonthView({ yearMonth }: MonthViewProps) {
  const startDate = `${yearMonth}-01`;
  const endDate = monthEndISO(yearMonth);

  const { data: summaries } = useQuery<ProductivitySummary[]>(
    "productivity_summary_range",
    { start_date: startDate, end_date: endDate },
    [],
  );

  const { totalActive, totalFocus, totalBreak, topCats } = useMemo(() => {
    const active = summaries.reduce((s, d) => s + d.totalActiveSecs, 0);
    const focus = summaries.reduce((s, d) => s + d.totalFocusSecs, 0);
    const brk = summaries.reduce((s, d) => s + d.totalBreakSecs, 0);

    const allCats = new Map<string, number>();
    for (const s of summaries) {
      for (const c of s.topCategories) {
        allCats.set(c.category, (allCats.get(c.category) ?? 0) + c.durationSecs);
      }
    }
    const cats = Array.from(allCats.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([category, durationSecs]) => ({ category, durationSecs }));

    return { totalActive: active, totalFocus: focus, totalBreak: brk, topCats: cats };
  }, [summaries]);

  const breakdownSegments = useMemo(
    () => buildBreakdownSegments(totalActive, totalFocus, totalBreak),
    [totalActive, totalFocus, totalBreak],
  );

  return (
    <div className="grid grid-cols-3 gap-4 auto-rows-min">
      <MonthlyChart summaries={summaries} />
      <CategoriesList categories={topCats} totalSecs={totalActive} />
      <BreakdownDonuts segments={breakdownSegments} totalSecs={totalActive} />
      <MonthlyStats yearMonth={yearMonth} summaries={summaries} />
    </div>
  );
}
