import { useMemo } from "react";
import { useQuery } from "@shared/hooks/useQuery";
import { monthEndISO } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { BreakdownDonuts } from "./BreakdownDonuts";
import { CategoriesList } from "./CategoriesList";
import { MonthlyChart } from "./MonthlyChart";
import { MonthlyStats } from "./MonthlyStats";
import { buildBreakdownSegments } from "../lib/constants";

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
