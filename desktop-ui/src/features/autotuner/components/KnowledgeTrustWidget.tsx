import { useQuery } from "@shared/hooks/useQuery";
import { formatRelativeTime } from "@shared/lib/dates";

import type { MemoryHealthResponse } from "../types";

function scoreColor(score: number): string {
  if (score >= 0.85) return "bg-green-500/15 text-green-400 border-green-500/30";
  if (score >= 0.7) return "bg-amber-500/15 text-amber-400 border-amber-500/30";
  return "bg-red-500/15 text-red-400 border-red-500/30";
}

function trendArrow(delta: number | null): string {
  if (delta == null) return "";
  const points = Math.abs(Math.round(delta * 100));
  if (delta > 0) return ` ↑${points}%`;
  if (delta < 0) return ` ↓${points}%`;
  return "";
}

function trendColor(pct: number | null): string {
  if (pct == null) return "";
  return pct >= 0 ? "text-green-400" : "text-red-400";
}

export function KnowledgeTrustWidget() {
  const { data, loading } = useQuery<MemoryHealthResponse>("memory_health");

  if (loading) {
    return <div className="glass-card p-4 h-28 animate-pulse" />;
  }

  if (!data || data.totalFacts90d === 0) {
    return null;
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-baseline justify-between">
        <div>
          <h3 className="text-[13px] font-medium text-foreground">Knowledge Trust</h3>
          <p className="text-[11px] text-dim">How well I know you</p>
        </div>
        <div className="text-right">
          <span className="text-2xl font-semibold text-foreground">
            {Math.round(data.overall * 100)}%
          </span>
          {data.trendPct != null && (
            <span className={`text-[11px] ml-1 ${trendColor(data.trendPct)}`}>
              {trendArrow(data.trendPct)}
            </span>
          )}
        </div>
      </div>

      <div className="flex flex-wrap gap-1.5">
        {data.domains.map((d) => (
          <span
            key={d.domain}
            className={`px-2 py-0.5 rounded-full text-[11px] font-medium border ${scoreColor(d.score)}`}
            title={`${d.totalFacts} facts, ${d.fastFailures} fast failures`}
          >
            {d.domain}: {Math.round(d.score * 100)}%
          </span>
        ))}
      </div>

      <p className="text-2xs text-dim">
        {data.totalFacts90d} facts tracked
        {data.computedAt && ` · updated ${formatRelativeTime(data.computedAt)}`}
      </p>
    </div>
  );
}
