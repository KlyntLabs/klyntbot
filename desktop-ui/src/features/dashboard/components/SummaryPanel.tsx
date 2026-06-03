import Brain from "lucide-react/dist/esm/icons/brain";
import ExternalLink from "lucide-react/dist/esm/icons/external-link";
import Lightbulb from "lucide-react/dist/esm/icons/lightbulb";
import X from "lucide-react/dist/esm/icons/x";
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
import { cn } from "@/utils/cn";
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
    staleTime: 5 * 60_000,
    fallback: null,
  });

  const { data: weeklyData } = useTauriQuery<ProductivitySummaryResponse[]>({
    queryKey: qk.productivity.weekly(),
    queryFn: () => productivityWeeklyQuery(),
    staleTime: 5 * 60_000,
    fallback: [],
  });

  return (
    <aside className="w-80 shrink-0 px-4 py-3 flex flex-col gap-3 overflow-y-auto bg-surface-messages border-l border-border-subtle text-[var(--fs-base)]">
      {hasProductivity && ps.productivityScore != null && (
        <section className="flex flex-col gap-1.5">
          <div className="flex items-start gap-3">
            <ProductivityScoreRing score={ps.productivityScore} size={72} />
            <div className="flex-1 min-w-0 flex flex-col gap-1.5">
              <div className="flex items-center gap-1.5">
                <span className="text-ui-sm font-semibold text-ds-text-strong tabular-nums">
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
                <span className="text-ui-2xs text-ds-text-subtle">active</span>
              </div>
              <div className="flex h-1 rounded-full overflow-hidden bg-surface-control">
                {ps.productiveSecs > 0 && (
                  <div
                    className="h-full bg-success"
                    style={{ width: `${(ps.productiveSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
                {ps.neutralSecs > 0 && (
                  <div
                    className="h-full bg-ds-text-subtle"
                    style={{ width: `${(ps.neutralSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
                {ps.distractingSecs > 0 && (
                  <div
                    className="h-full bg-destructive"
                    style={{ width: `${(ps.distractingSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
              </div>
              <span className="text-ui-2xs text-success">{productivePct}% productive</span>
              {ps.totalActiveSecs > 0 && (
                <div className="flex flex-col gap-0.5 mt-1">
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
                <div className="flex justify-between text-ui-2xs text-ds-text-subtle px-1">
                  <span>
                    {ps.deepWorkBlocks} deep work block{ps.deepWorkBlocks !== 1 ? "s" : ""}
                  </span>
                  <span>{formatHumanDuration(ps.deepWorkSecs)}</span>
                </div>
              )}
              {ps.avgRecoverySecs != null && (
                <div className="flex justify-between text-ui-2xs text-ds-text-subtle px-1">
                  <span>Avg recovery</span>
                  <span>{Math.round(ps.avgRecoverySecs)}s</span>
                </div>
              )}
            </div>
          </div>
        </section>
      )}

      {!hasProductivity && (
        <section className="flex flex-col gap-1.5">
          <div className="flex items-center gap-1.5">
            <span className="text-ui-sm font-semibold text-ds-text-strong tabular-nums">
              {formatHumanDuration(summary.totalTrackedSecs)}
            </span>
            <span className="text-ui-2xs text-ds-text-subtle">tracked</span>
          </div>
        </section>
      )}

      {intel?.focusRecommendation && (
        <p className="text-ui-2xs italic text-ds-text-subtle leading-relaxed">{intel.focusRecommendation}</p>
      )}

      {weeklyData && weeklyData.length >= 2 ? (
        <WeeklySparkline data={weeklyData} />
      ) : (
        <div className="flex items-center gap-3 opacity-70">
          <span className="text-ui-2xs italic text-[color-mix(in_srgb,var(--ds-text-subtle)_70%,transparent)]">
            Weekly trend appears after 2+ days of tracking.
          </span>
        </div>
      )}

      <PatternsCard />

      <HourlyHeatmap startDate={date} endDate={date} />

      {hasProductivity && ps.topApps.length > 0 && (
        <section className="flex flex-col gap-1.5">
          <h3 className="text-ui-2xs font-medium text-ds-text-subtle uppercase tracking-wider mb-1.5">Top Apps</h3>
          <TopAppsChart apps={ps.topApps} />
        </section>
      )}

      {!hasProductivity && summary.topApps.length > 0 && (
        <section className="flex flex-col gap-1.5">
          <h3 className="text-ui-2xs font-medium text-ds-text-subtle uppercase tracking-wider mb-1.5">Top Apps</h3>
          <TopAppsChart
            apps={summary.topApps.map((a) => ({
              appName: a.appName,
              durationSecs: a.durationSecs,
              category: null,
            }))}
          />
        </section>
      )}

      <section className="flex flex-col gap-1.5">
        <h3 className="text-ui-2xs font-medium text-ds-text-subtle uppercase tracking-wider mb-1.5">Insights</h3>
        {intel && (intel.patterns.length > 0 || intel.nudges.length > 0) ? (
          <div className="flex flex-col gap-2">
            {intel.patterns.map((p) => (
              <div key={`p-${p}`} className="flex items-start gap-1.5 text-ui-2xs text-ds-text-subtle">
                <Brain aria-hidden className="w-3 h-3 mt-0.5 shrink-0" />
                <span>{p}</span>
              </div>
            ))}
            {intel.nudges.map((n) => (
              <div
                key={`n-${n.nudgeType}-${n.message}`}
                className="flex items-start gap-1.5 text-ui-2xs text-ds-text-subtle"
              >
                <Lightbulb aria-hidden className="w-3 h-3 mt-0.5 shrink-0" />
                <span>{n.message}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="opacity-70">
            <span className="text-ui-2xs italic text-[color-mix(in_srgb,var(--ds-text-subtle)_70%,transparent)]">
              Insights appear once we detect patterns and nudges.
            </span>
          </div>
        )}
      </section>

      {ps?.aiSummary && (
        <section className="border border-[color-mix(in_srgb,var(--brand)_15%,transparent)] rounded-lg px-2.5 py-2 bg-[color-mix(in_srgb,var(--brand)_6%,transparent)]">
          <p className="text-ui-2xs text-ds-text-subtle leading-relaxed m-0">{ps.aiSummary}</p>
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
    <div className="flex flex-col gap-1">
      {apps.slice(0, 5).map((app) => {
        const pct = maxSecs > 0 ? (app.durationSecs / maxSecs) * 100 : 0;
        const color = getAppColor(app.appName, app.category ?? null);
        return (
          <div key={app.appName} className="flex items-center gap-1.5">
            <span className="text-ui-2xs text-ds-text-subtle w-16 shrink-0 whitespace-nowrap overflow-hidden text-ellipsis" title={app.appName}>
              {app.appName}
            </span>
            <div className="flex-1 h-1 rounded-full bg-surface-control overflow-hidden">
              <div
                className="h-full rounded-full"
                style={{
                  width: `${Math.max(pct, 4)}%`,
                  backgroundColor: color,
                  opacity: 0.6 + (pct / 100) * 0.4,
                }}
              />
            </div>
            <span className="text-ui-2xs text-ds-text-subtle tabular-nums text-right shrink-0 whitespace-nowrap">
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

  return (
    <div className="flex items-center gap-3">
      <svg
        width={w}
        height={h}
        className="flex-1"
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
        <span className={cn("text-ui-2xs font-medium shrink-0", changePct > 0 ? "text-success" : "text-destructive")}>
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
    <aside className="w-80 shrink-0 px-4 py-3 flex flex-col gap-3 overflow-y-auto bg-surface-messages border-l border-border-subtle text-[var(--fs-base)]">
      <div className="flex items-center justify-between">
        <h2 className="text-ui-2xs font-medium text-ds-text-subtle uppercase tracking-wider mb-1.5">Activity Session</h2>
        <button
          type="button"
          onClick={onClose}
          className="bg-none border-none text-ds-text-subtle cursor-pointer p-1 hover:text-ds-text-strong"
          aria-label="Close details"
        >
          <X aria-hidden className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="flex items-center gap-2 text-[var(--fs-base)] font-medium text-ds-text-strong">
        <div
          className="w-3 h-3 rounded-sm shrink-0"
          style={{ backgroundColor: session.color }}
        />
        <span>{session.label}</span>
      </div>

      {matched?.description && (
        <p className="text-ui-2xs text-ds-text-subtle leading-relaxed m-0">{matched.description}</p>
      )}

      <div className="flex items-center gap-1.5 flex-wrap">
        {matched?.qualityScore != null && (
          <div
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-ui-2xs font-medium"
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
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-ui-2xs font-medium"
          style={{
            backgroundColor: `color-mix(in oklch, ${categoryColor} 15%, transparent)`,
            color: categoryColor,
            border: `1px solid color-mix(in oklch, ${categoryColor} 25%, transparent)`,
          }}
        >
          <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: categoryColor }} />
          {categoryLabel}
        </div>
      </div>

      {matched && (
        <div className="flex flex-col gap-0.5 text-ui-2xs text-ds-text-subtle">
          {matched.categoryPurity != null && (
            <div>Focus purity: {Math.round(matched.categoryPurity * 100)}%</div>
          )}
          <div>Context switches: {matched.contextSwitches}</div>
          <div>Distractions: {matched.distractionCount}</div>
        </div>
      )}

      <div className="flex flex-col gap-0.5 text-ui-2xs text-ds-text-subtle">
        <div>
          {fmt(startH, startM)} – {fmt(endH, endM)}
        </div>
        <div>Duration: {formatHumanDuration(session.duration)}</div>
      </div>

      {session.appBreakdown.length > 0 && (
        <div>
          <h3 className="text-ui-2xs font-medium text-ds-text-subtle uppercase tracking-wider mb-1.5">Apps in this session</h3>
          <div>
            {session.appBreakdown.map((app) => {
              const appCatColor = resolveActivityColor(app.catType, false);
              return (
                <div key={app.app} className="flex items-center gap-1.5 mb-1">
                  <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ backgroundColor: appCatColor }} />
                  <span className="text-ui-2xs text-ds-text-subtle flex-1 whitespace-nowrap overflow-hidden text-ellipsis">{app.app}</span>
                  <span className="text-ui-2xs text-ds-text-subtle tabular-nums">{formatHumanDuration(app.dur)}</span>
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
    <aside className="w-80 shrink-0 px-4 py-3 flex flex-col gap-3 overflow-y-auto bg-surface-messages border-l border-border-subtle text-[var(--fs-base)]">
      <div className="flex items-center justify-between">
        <h2 className="text-ui-2xs font-medium text-ds-text-subtle uppercase tracking-wider mb-1.5">Details</h2>
        <button
          type="button"
          onClick={onClose}
          className="bg-none border-none text-ds-text-subtle cursor-pointer p-1 hover:text-ds-text-strong"
          aria-label="Close details"
        >
          <X aria-hidden className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="flex items-center gap-1.5 text-[var(--fs-base)] font-medium text-ds-text-strong">
        <div
          className="w-3 h-3 rounded-sm shrink-0"
          style={{ backgroundColor: entry.color }}
        />
        <span>{entry.title}</span>
      </div>

      {entry.description && <p className="text-ui-2xs text-ds-text-subtle leading-relaxed m-0">{entry.description}</p>}

      <div className="flex flex-col gap-0.5 text-ui-2xs text-ds-text-subtle">
        <div>Started: {new Date(entry.startedAt).toLocaleTimeString()}</div>
        {entry.endedAt && <div>Ended: {new Date(entry.endedAt).toLocaleTimeString()}</div>}
        {entry.durationSecs != null && entry.durationSecs > 0 && (
          <div>Duration: {formatHumanDuration(entry.durationSecs)}</div>
        )}
        <div className="capitalize">Source: {entry.source}</div>
      </div>

      {entry.entityRoute && (
        <a
          href={entry.entityRoute}
          className="inline-flex items-center gap-1.5 text-ui-2xs text-brand no-underline mt-1 hover:underline"
          onClick={(e) => e.preventDefault()}
        >
          <ExternalLink aria-hidden className="w-3 h-3" />
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
  const ariaLabel = `${isUp ? "Up" : "Down"} ${pct}%${label ? ` ${label}` : ""}`;
  return (
    <span
      className={cn("text-ui-2xs font-medium", isUp ? "text-success" : "text-destructive")}
      title={label ? `${isUp ? "+" : "-"}${pct}% ${label}` : undefined}
      aria-label={ariaLabel}
    >
      {isUp ? "↑" : "↓"}
      {pct > 0 && `${pct}%`}
    </span>
  );
}

// Re-export for use by views that fetch productivity-summary data and pass it down.
export type { SessionBlock };
export { productivitySummaryRangeQuery, productivityTodayQuery };
