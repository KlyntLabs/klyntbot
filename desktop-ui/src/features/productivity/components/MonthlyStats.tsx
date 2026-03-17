import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, monthEndISO, shiftMonth } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { useMemo } from "react";

interface MonthlyStatsProps {
  yearMonth: string;
  summaries: ProductivitySummary[];
}

function delta(current: number, previous: number): { text: string; positive: boolean | null } {
  const diff = current - previous;
  if (diff === 0) return { text: "—", positive: null };
  const sign = diff > 0 ? "+" : "";
  return { text: `${sign}${formatHumanDuration(Math.abs(diff))}`, positive: diff > 0 };
}

function scoreDelta(current: number, previous: number): { text: string; positive: boolean | null } {
  const diff = Math.round(current - previous);
  if (diff === 0) return { text: "—", positive: null };
  return { text: diff > 0 ? `+${diff}` : `${diff}`, positive: diff > 0 };
}

export function MonthlyStats({ yearMonth, summaries: current }: MonthlyStatsProps) {
  const prevMonth = shiftMonth(yearMonth, -1);
  const prevStart = `${prevMonth}-01`;
  const prevEnd = monthEndISO(prevMonth);

  const { data: previous } = useQuery<ProductivitySummary[]>(
    "productivity_summary_range",
    { start_date: prevStart, end_date: prevEnd },
    [],
  );

  const rows = useMemo(() => {
    const curDays = current.length || 1;
    const prevDays = previous.length || 1;

    const curActive = current.reduce((s, d) => s + d.totalActiveSecs, 0);
    const prevActive = previous.reduce((s, d) => s + d.totalActiveSecs, 0);
    const curAvgDaily = Math.round(curActive / curDays);
    const prevAvgDaily = Math.round(prevActive / prevDays);
    const curAvgWeekly = Math.round((curActive / curDays) * 7);
    const prevAvgWeekly = Math.round((prevActive / prevDays) * 7);

    const curScores = current.map((s) => s.productivityScore).filter((s): s is number => s != null);
    const prevScores = previous
      .map((s) => s.productivityScore)
      .filter((s): s is number => s != null);
    const curAvgScore =
      curScores.length > 0 ? curScores.reduce((a, b) => a + b, 0) / curScores.length : 0;
    const prevAvgScore =
      prevScores.length > 0 ? prevScores.reduce((a, b) => a + b, 0) / prevScores.length : 0;

    return [
      {
        label: "Avg. Weekly Hours",
        value: formatHumanDuration(curAvgWeekly),
        prev: formatHumanDuration(prevAvgWeekly),
        change: delta(curAvgWeekly, prevAvgWeekly),
      },
      {
        label: "Avg. Daily Hours",
        value: formatHumanDuration(curAvgDaily),
        prev: formatHumanDuration(prevAvgDaily),
        change: delta(curAvgDaily, prevAvgDaily),
      },
      {
        label: "Avg. Score",
        value: `${Math.round(curAvgScore)}/100`,
        prev: `${Math.round(prevAvgScore)}`,
        change: scoreDelta(curAvgScore, prevAvgScore),
      },
    ];
  }, [current, previous]);

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Work Hours</h2>
      <div className="flex flex-col gap-0">
        {rows.map((row, i) => (
          <div
            key={row.label}
            className={`flex flex-col gap-1 py-3 ${i > 0 ? "border-t border-border-subtle" : ""}`}
          >
            <span className="text-[10px] font-light text-dim uppercase tracking-wider">
              {row.label}
            </span>
            <span className="text-[20px] font-light text-primary tabular-nums leading-tight">
              {row.value}
            </span>
            <div className="flex items-center gap-2 text-[10px] font-light">
              <span className="text-dim">vs {row.prev}</span>
              {row.change.positive !== null && (
                <span
                  style={{
                    color: row.change.positive ? "var(--success)" : "var(--destructive)",
                  }}
                >
                  {row.change.text}
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
