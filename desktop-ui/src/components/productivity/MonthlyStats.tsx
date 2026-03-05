import { useQuery } from '../../hooks/useQuery';
import { formatHumanDuration, shiftMonth, monthEndISO } from '../../lib/dates';
import type { ProductivitySummary } from '../../lib/types';

interface MonthlyStatsProps {
  yearMonth: string;
  summaries: ProductivitySummary[];
}

function delta(current: number, previous: number): string {
  const diff = current - previous;
  if (diff === 0) return '—';
  const sign = diff > 0 ? '+' : '';
  return `${sign}${formatHumanDuration(Math.abs(diff))}`;
}

function scoreDelta(current: number, previous: number): string {
  const diff = Math.round(current - previous);
  if (diff === 0) return '—';
  return diff > 0 ? `+${diff}` : `${diff}`;
}

export function MonthlyStats({ yearMonth, summaries: current }: MonthlyStatsProps) {
  const prevMonth = shiftMonth(yearMonth, -1);
  const prevStart = `${prevMonth}-01`;
  const prevEnd = monthEndISO(prevMonth);

  const { data: previous } = useQuery<ProductivitySummary[]>(
    'productivity_summary_range',
    { start_date: prevStart, end_date: prevEnd },
    [],
  );

  const curDays = current.length || 1;
  const prevDays = previous.length || 1;

  const curActive = current.reduce((s, d) => s + d.totalActiveSecs, 0);
  const prevActive = previous.reduce((s, d) => s + d.totalActiveSecs, 0);
  const curAvgDaily = Math.round(curActive / curDays);
  const prevAvgDaily = Math.round(prevActive / prevDays);
  const curAvgWeekly = Math.round((curActive / curDays) * 7);
  const prevAvgWeekly = Math.round((prevActive / prevDays) * 7);

  const curScores = current.map((s) => s.productivityScore).filter((s): s is number => s != null);
  const prevScores = previous.map((s) => s.productivityScore).filter((s): s is number => s != null);
  const curAvgScore = curScores.length > 0 ? curScores.reduce((a, b) => a + b, 0) / curScores.length : 0;
  const prevAvgScore = prevScores.length > 0 ? prevScores.reduce((a, b) => a + b, 0) / prevScores.length : 0;

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Work Hours</h2>
      <div className="flex flex-col gap-3">
        <div>
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-light text-dim">Avg. Work Hours per week</span>
          </div>
          <span className="text-[22px] font-light text-primary tabular-nums">{formatHumanDuration(curAvgWeekly)}</span>
          <div className="flex gap-2 text-[10px] font-light text-dim">
            <span>Last month: {formatHumanDuration(prevAvgWeekly)}</span>
            <span>Change: {delta(curAvgWeekly, prevAvgWeekly)}</span>
          </div>
        </div>
        <div className="border-t border-border-subtle pt-3">
          <span className="text-[10px] font-light text-dim">Avg. time worked per day</span>
          <div className="text-[18px] font-light text-primary tabular-nums">{formatHumanDuration(curAvgDaily)}</div>
          <div className="flex gap-2 text-[10px] font-light text-dim">
            <span>Last month: {formatHumanDuration(prevAvgDaily)}</span>
            <span>Change: {delta(curAvgDaily, prevAvgDaily)}</span>
          </div>
        </div>
        <div className="border-t border-border-subtle pt-3">
          <span className="text-[10px] font-light text-dim">Avg. Score</span>
          <div className="text-[18px] font-light text-primary tabular-nums">{Math.round(curAvgScore)}/100</div>
          <div className="flex gap-2 text-[10px] font-light text-dim">
            <span>Last month: {Math.round(prevAvgScore)}</span>
            <span>Change: {scoreDelta(curAvgScore, prevAvgScore)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
