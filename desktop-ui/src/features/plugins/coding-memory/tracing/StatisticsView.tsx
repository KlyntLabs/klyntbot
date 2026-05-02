import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { ErrorByTool, ProjectTotals, StatsBundle, ToolUsage } from "./types";

interface Props {
  providerId: string;
}

export function StatisticsView({ providerId }: Props) {
  const [stats, setStats] = useState<StatsBundle | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    invoke<StatsBundle>("tracing_stats", { providerId })
      .then((s) => {
        if (!cancelled) setStats(s);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [providerId]);

  if (loading) return <div className="tracing-state">Loading statistics…</div>;
  if (!stats) return <div className="tracing-state">No statistics available.</div>;

  const totalSessions = stats.perProject.reduce((acc, p) => acc + p.sessionCount, 0);
  const totalTurns = stats.perProject.reduce((acc, p) => acc + p.turnCount, 0);
  const totalInputTokens = stats.perProject.reduce((acc, p) => acc + p.totalInputTokens, 0);
  const totalOutputTokens = stats.perProject.reduce((acc, p) => acc + p.totalOutputTokens, 0);

  return (
    <div className="tracing-statistics">
      <div className="tracing-statistics__row">
        <div className="tracing-statistics__card">
          <div className="tracing-statistics__value">{totalSessions}</div>
          <div className="tracing-statistics__label">Sessions</div>
        </div>
        <div className="tracing-statistics__card">
          <div className="tracing-statistics__value">{totalTurns}</div>
          <div className="tracing-statistics__label">Turns</div>
        </div>
        <div className="tracing-statistics__card">
          <div className="tracing-statistics__value">{(totalInputTokens / 1000).toFixed(1)}k</div>
          <div className="tracing-statistics__label">Input tokens</div>
        </div>
        <div className="tracing-statistics__card">
          <div className="tracing-statistics__value">{(totalOutputTokens / 1000).toFixed(1)}k</div>
          <div className="tracing-statistics__label">Output tokens</div>
        </div>
        <div className="tracing-statistics__card">
          <div className="tracing-statistics__value">{stats.cacheHitPct.toFixed(0)}%</div>
          <div className="tracing-statistics__label">Cache hit</div>
        </div>
      </div>

      <h4 className="tracing-statistics__section">Projects</h4>
      <div className="tracing-statistics__table">
        {stats.perProject.map((p: ProjectTotals) => (
          <div key={p.projectBasename} className="tracing-statistics__row">
            <span className="tracing-statistics__cell">{p.projectBasename}</span>
            <span className="tracing-statistics__cell">{p.sessionCount} sessions</span>
            <span className="tracing-statistics__cell">{p.turnCount} turns</span>
          </div>
        ))}
      </div>

      <h4 className="tracing-statistics__section">Tool Usage</h4>
      <div className="tracing-statistics__table">
        {stats.toolUsage.map((t: ToolUsage) => (
          <div key={t.tool} className="tracing-statistics__row">
            <span className="tracing-statistics__cell">{t.tool}</span>
            <span className="tracing-statistics__cell">{t.callCount} calls</span>
            <span className="tracing-statistics__cell">{t.errorCount} errors</span>
          </div>
        ))}
      </div>

      <h4 className="tracing-statistics__section">Errors</h4>
      <div className="tracing-statistics__table">
        {stats.errorsByTool.map((e: ErrorByTool) => (
          <div key={e.tool} className="tracing-statistics__row">
            <span className="tracing-statistics__cell">{e.tool}</span>
            <span className="tracing-statistics__cell">{e.errorCount} errors</span>
          </div>
        ))}
      </div>
    </div>
  );
}
