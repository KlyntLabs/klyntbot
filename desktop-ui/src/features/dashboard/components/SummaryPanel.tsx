import { Brain, ExternalLink, Lightbulb, X } from "lucide-react";
import {
  dashboardIntelligenceQuery,
  productivitySummaryRangeQuery,
  productivityTodayQuery,
  productivityWeeklyQuery,
} from "@/api/endpoints/dashboard";
import type {
  DashboardIntelligenceResponse,
  ProductivitySummaryResponse,
  TimelineEntry,
  TimelineSummary,
} from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, TZ_OFFSET_MINS, todayISO } from "@/utils/dashboardDates";
import { getAppColor, resolveActivityColor, resolveCategoryLabel } from "../lib/productivity";
import { GoalsProgress } from "./productivity/GoalsProgress";
import { HourlyHeatmap } from "./productivity/HourlyHeatmap";
import { PatternsCard } from "./productivity/PatternsCard";
import { ProductivityScoreRing, ScoreBar } from "./productivity/ProductivityScoreRing";
import type { SessionBlock } from "./views/ActivityTrack";

interface SummaryPanelProps {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  selectedSession?: SessionBlock | null;
  onClose: () => void;
  productivitySummary?: ProductivitySummaryResponse | null;
  date?: string;
}

export function SummaryPanel({
  summary,
  selectedEntry,
  selectedSession,
  onClose,
  productivitySummary,
  date,
}: SummaryPanelProps) {
  if (selectedSession) {
    return <SessionDetail session={selectedSession} onClose={onClose} />;
  }
  if (selectedEntry) {
    return <EntryDetail entry={selectedEntry} onClose={onClose} />;
  }
  if (!summary) return null;
  return (
    <DaySummary
      summary={summary}
      productivitySummary={productivitySummary}
      date={date || todayISO()}
    />
  );
}

function DaySummary({
  summary,
  productivitySummary,
  date,
}: {
  summary: TimelineSummary;
  productivitySummary?: ProductivitySummaryResponse | null;
  date: string;
}) {
  const ps = productivitySummary;
  const hasProductivity = ps != null && ps.totalActiveSecs > 0;
  const productivePct = hasProductivity
    ? Math.round((ps.productiveSecs / ps.totalActiveSecs) * 100)
    : 0;

  const { data: intel } = useTauriQuery<DashboardIntelligenceResponse | null>({
    queryKey: qk.dashboard.intelligence(date),
    queryFn: () => dashboardIntelligenceQuery(date, TZ_OFFSET_MINS),
    fallback: null,
  });

  const { data: weeklyData } = useTauriQuery<ProductivitySummaryResponse[]>({
    queryKey: qk.productivity.weekly(),
    queryFn: () => productivityWeeklyQuery(),
    fallback: [],
  });

  return (
    <aside className="dashboard__summary-panel">
      {hasProductivity && ps.productivityScore != null && (
        <section className="dashboard__summary-section">
          <div className="dashboard__summary-score-row">
            <ProductivityScoreRing score={ps.productivityScore} size={72} />
            <div className="dashboard__summary-score-meta">
              <div className="dashboard__summary-active">
                <span className="dashboard__summary-active-time">
                  {formatHumanDuration(ps.totalActiveSecs)}
                </span>
                <TrendArrow
                  value={
                    ps.activeTimeTrend != null
                      ? (() => {
                          const baseline = ps.totalActiveSecs - ps.activeTimeTrend;
                          if (baseline < 60) return null;
                          return (ps.activeTimeTrend / baseline) * 100;
                        })()
                      : null
                  }
                />
                <span className="dashboard__summary-dim">active</span>
              </div>
              <div className="dashboard__summary-bar">
                {ps.productiveSecs > 0 && (
                  <div
                    className="dashboard__summary-bar-seg dashboard__summary-bar-seg--productive"
                    style={{ width: `${(ps.productiveSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
                {ps.neutralSecs > 0 && (
                  <div
                    className="dashboard__summary-bar-seg dashboard__summary-bar-seg--neutral"
                    style={{ width: `${(ps.neutralSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
                {ps.distractingSecs > 0 && (
                  <div
                    className="dashboard__summary-bar-seg dashboard__summary-bar-seg--distracting"
                    style={{ width: `${(ps.distractingSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
              </div>
              <span className="dashboard__summary-productive-pct">{productivePct}% productive</span>
              {ps.totalActiveSecs > 0 && (
                <div className="dashboard__summary-metrics">
                  <ScoreBar label="Deep focus" value={ps.productiveSecs / ps.totalActiveSecs} />
                  <ScoreBar label="Quality" value={ps.avgSessionQuality ?? 0} />
                  <ScoreBar
                    label="Low distraction"
                    value={1 - ps.distractingSecs / Math.max(ps.totalActiveSecs, 1)}
                  />
                  <ScoreBar
                    label="Alignment"
                    value={ps.contextSwitches > 0 ? Math.max(0, 1 - ps.contextSwitches / 100) : 1}
                  />
                </div>
              )}
              {ps.deepWorkBlocks > 0 && (
                <div className="dashboard__summary-stat-row">
                  <span>
                    {ps.deepWorkBlocks} deep work block{ps.deepWorkBlocks !== 1 ? "s" : ""}
                  </span>
                  <span>{formatHumanDuration(ps.deepWorkSecs)}</span>
                </div>
              )}
              {ps.avgRecoverySecs != null && (
                <div className="dashboard__summary-stat-row">
                  <span>Avg recovery</span>
                  <span>{Math.round(ps.avgRecoverySecs)}s</span>
                </div>
              )}
            </div>
          </div>
        </section>
      )}

      {!hasProductivity && (
        <section className="dashboard__summary-section">
          <div className="dashboard__summary-active">
            <span className="dashboard__summary-active-time">
              {formatHumanDuration(summary.totalTrackedSecs)}
            </span>
            <span className="dashboard__summary-dim">tracked</span>
          </div>
        </section>
      )}

      {intel?.focusRecommendation && (
        <p className="dashboard__summary-recommendation">{intel.focusRecommendation}</p>
      )}

      {weeklyData && weeklyData.length >= 2 ? (
        <WeeklySparkline data={weeklyData} />
      ) : (
        <div className="dashboard__sparkline dashboard__sparkline--empty">
          <span className="dashboard__sparkline-empty-msg">
            Weekly trend appears after 2+ days of tracking.
          </span>
        </div>
      )}

      {hasProductivity && <PatternsCard />}

      {hasProductivity && <HourlyHeatmap startDate={date} endDate={date} />}

      {hasProductivity && ps.topApps.length > 0 && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Top Apps</h4>
          <TopAppsChart apps={ps.topApps} />
        </section>
      )}

      {!hasProductivity && summary.topApps.length > 0 && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Top Apps</h4>
          <TopAppsChart
            apps={summary.topApps.map((a) => ({
              appName: a.appName,
              durationSecs: a.durationSecs,
              category: null,
            }))}
          />
        </section>
      )}

      {intel && (intel.patterns.length > 0 || intel.nudges.length > 0) && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Insights</h4>
          <div className="dashboard__summary-insights">
            {intel.patterns.map((p) => (
              <div key={`p-${p}`} className="dashboard__summary-insight-item">
                <Brain aria-hidden />
                <span>{p}</span>
              </div>
            ))}
            {intel.nudges.map((n) => (
              <div
                key={`n-${n.nudgeType}-${n.message}`}
                className="dashboard__summary-insight-item"
              >
                <Lightbulb aria-hidden />
                <span>{n.message}</span>
              </div>
            ))}
          </div>
        </section>
      )}

      {ps?.aiSummary && (
        <section className="dashboard__summary-aibox">
          <p>{ps.aiSummary}</p>
        </section>
      )}

      <GoalsProgress />
    </aside>
  );
}

function TopAppsChart({
  apps,
}: {
  apps: { appName: string; durationSecs: number; category?: string | null }[];
}) {
  const maxSecs = apps[0]?.durationSecs ?? 1;
  return (
    <div className="dashboard__summary-apps">
      {apps.slice(0, 5).map((app) => {
        const pct = maxSecs > 0 ? (app.durationSecs / maxSecs) * 100 : 0;
        const color = getAppColor(app.appName, app.category ?? null);
        return (
          <div key={app.appName} className="dashboard__summary-app-row">
            <span className="dashboard__summary-app-name" title={app.appName}>
              {app.appName}
            </span>
            <div className="dashboard__summary-app-track">
              <div
                className="dashboard__summary-app-fill"
                style={{
                  width: `${Math.max(pct, 4)}%`,
                  backgroundColor: color,
                  opacity: 0.6 + (pct / 100) * 0.4,
                }}
              />
            </div>
            <span className="dashboard__summary-app-dur">
              {formatHumanDuration(app.durationSecs)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function WeeklySparkline({ data }: { data: ProductivitySummaryResponse[] }) {
  const scores = data.map((d) => d.productivityScore ?? 0);
  if (scores.length < 2) return null;

  const halfLen = Math.floor(scores.length / 2);
  const recentAvg = scores.slice(halfLen).reduce((a, b) => a + b, 0) / (scores.length - halfLen);
  const olderAvg = scores.slice(0, halfLen).reduce((a, b) => a + b, 0) / halfLen;
  const changePct = olderAvg > 0 ? Math.round(((recentAvg - olderAvg) / olderAvg) * 100) : 0;

  const w = 200;
  const h = 32;
  const pad = 2;
  const max = Math.max(...scores, 1);
  const min = Math.min(...scores, 0);
  const range = max - min || 1;

  const points = scores
    .map((v, i) => {
      const x = pad + (i / (scores.length - 1)) * (w - pad * 2);
      const y = h - pad - ((v - min) / range) * (h - pad * 2);
      return `${x},${y}`;
    })
    .join(" ");

  const lastX = w - pad;
  const lastY = h - pad - ((scores[scores.length - 1] - min) / range) * (h - pad * 2);

  const trendClass =
    changePct > 0
      ? "dashboard__sparkline-trend dashboard__sparkline-trend--up"
      : "dashboard__sparkline-trend dashboard__sparkline-trend--down";

  return (
    <div className="dashboard__sparkline">
      <svg
        width={w}
        height={h}
        className="dashboard__sparkline-svg"
        role="img"
        aria-label="Weekly productivity trend"
      >
        <polyline
          points={points}
          fill="none"
          stroke="var(--brand)"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx={lastX} cy={lastY} r="2.5" fill="var(--brand)" />
      </svg>
      {changePct !== 0 && (
        <span className={trendClass}>
          {changePct > 0 ? "↑" : "↓"}
          {Math.abs(changePct)}%
        </span>
      )}
    </div>
  );
}

function SessionDetail({ session, onClose }: { session: SessionBlock; onClose: () => void }) {
  const startH = Math.floor(session.startMin / 60);
  const startM = Math.floor(session.startMin % 60);
  const endH = Math.floor(session.endMin / 60);
  const endM = Math.floor(session.endMin % 60);
  const fmt = (h: number, m: number) =>
    `${h % 12 || 12}:${String(m).padStart(2, "0")} ${h < 12 ? "AM" : "PM"}`;

  const categoryLabel = resolveCategoryLabel(session.dominantCategory);
  const categoryColor = resolveActivityColor(session.dominantCategory, false);
  const matched = session.intelligence;

  return (
    <aside className="dashboard__summary-panel">
      <div className="dashboard__summary-detail-header">
        <h3 className="dashboard__summary-heading">Activity Session</h3>
        <button
          type="button"
          onClick={onClose}
          className="dashboard__summary-close"
          aria-label="Close details"
        >
          <X aria-hidden />
        </button>
      </div>

      <div className="dashboard__summary-session-header">
        <div
          className="dashboard__summary-entry-swatch"
          style={{ backgroundColor: session.color }}
        />
        <span>{session.label}</span>
      </div>

      {matched?.description && (
        <p className="dashboard__summary-entry-desc">{matched.description}</p>
      )}

      <div className="dashboard__summary-session-stats">
        {matched?.qualityScore != null && (
          <div
            className="dashboard__summary-session-quality-badge"
            style={{
              backgroundColor: `color-mix(in oklch, ${session.color} 20%, transparent)`,
              color: session.color,
              border: `1px solid color-mix(in oklch, ${session.color} 30%, transparent)`,
            }}
          >
            Q: {Math.round(matched.qualityScore)}/100
          </div>
        )}
        <div
          className="dashboard__summary-session-category-badge"
          style={{
            backgroundColor: `color-mix(in oklch, ${categoryColor} 15%, transparent)`,
            color: categoryColor,
            border: `1px solid color-mix(in oklch, ${categoryColor} 25%, transparent)`,
          }}
        >
          <span style={{ backgroundColor: categoryColor }} />
          {categoryLabel}
        </div>
      </div>

      {matched && (
        <div className="dashboard__summary-entry-meta">
          {matched.categoryPurity != null && (
            <div>Focus purity: {Math.round(matched.categoryPurity * 100)}%</div>
          )}
          <div>Context switches: {matched.contextSwitches}</div>
          <div>Distractions: {matched.distractionCount}</div>
        </div>
      )}

      <div className="dashboard__summary-entry-meta">
        <div>
          {fmt(startH, startM)} – {fmt(endH, endM)}
        </div>
        <div>Duration: {formatHumanDuration(session.duration)}</div>
      </div>

      {session.appBreakdown.length > 0 && (
        <div>
          <h4 className="dashboard__summary-heading">Apps in this session</h4>
          <div>
            {session.appBreakdown.map((app) => {
              const appCatColor = resolveActivityColor(app.catType, false);
              return (
                <div key={app.app} className="dashboard__summary-session-app-row">
                  <span style={{ backgroundColor: appCatColor }} />
                  <span>{app.app}</span>
                  <span>{formatHumanDuration(app.dur)}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </aside>
  );
}

function EntryDetail({ entry, onClose }: { entry: TimelineEntry; onClose: () => void }) {
  return (
    <aside className="dashboard__summary-panel">
      <div className="dashboard__summary-detail-header">
        <h3 className="dashboard__summary-heading">Details</h3>
        <button
          type="button"
          onClick={onClose}
          className="dashboard__summary-close"
          aria-label="Close details"
        >
          <X aria-hidden />
        </button>
      </div>

      <div className="dashboard__summary-entry-title">
        <span
          className="dashboard__summary-entry-swatch"
          style={{ backgroundColor: entry.color }}
        />
        <span>{entry.title}</span>
      </div>

      {entry.description && <p className="dashboard__summary-entry-desc">{entry.description}</p>}

      <div className="dashboard__summary-entry-meta">
        <div>Started: {new Date(entry.startedAt).toLocaleTimeString()}</div>
        {entry.endedAt && <div>Ended: {new Date(entry.endedAt).toLocaleTimeString()}</div>}
        {entry.durationSecs != null && entry.durationSecs > 0 && (
          <div>Duration: {formatHumanDuration(entry.durationSecs)}</div>
        )}
        <div className="dashboard__summary-entry-source">Source: {entry.source}</div>
      </div>

      {entry.entityRoute && (
        <a
          href={entry.entityRoute}
          className="dashboard__summary-entry-link"
          onClick={(e) => e.preventDefault()}
        >
          <ExternalLink aria-hidden />
          <span>Open {entry.source}</span>
        </a>
      )}
    </aside>
  );
}

function TrendArrow({ value, label }: { value?: number | null; label?: string }) {
  if (value == null || Math.abs(value) < 0.5) return null;
  const isUp = value > 0;
  const pct = Math.round(Math.abs(value));
  const cls = isUp
    ? "dashboard__sparkline-trend dashboard__sparkline-trend--up"
    : "dashboard__sparkline-trend dashboard__sparkline-trend--down";
  return (
    <span className={cls} title={label ? `${isUp ? "+" : "-"}${pct}% ${label}` : undefined}>
      {isUp ? "↑" : "↓"}
      {pct > 0 && `${pct}%`}
    </span>
  );
}

// Re-export for use by views that fetch productivity-summary data and pass it down.
export type { SessionBlock };
export { productivitySummaryRangeQuery, productivityTodayQuery };
