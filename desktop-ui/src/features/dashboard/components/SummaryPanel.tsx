import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, TZ_OFFSET_MINS, todayISO } from "@shared/lib/dates";
import { getAppColor, resolveActivityColor, resolveCategoryLabel } from "@shared/lib/productivity";
import type { ProductivitySummary, TimelineEntry, TimelineSummary } from "@shared/types";
import { Brain, ExternalLink, Lightbulb, X } from "lucide-react";
import { useNavigate } from "react-router";
import type { SessionBlock } from "./ActivityTrack";
import { GoalsProgress } from "./productivity/GoalsProgress";
import { HourlyHeatmap } from "./productivity/HourlyHeatmap";
import { PatternsCard } from "./productivity/PatternsCard";
import { ProductivityScoreRing, ScoreBar } from "./productivity/ProductivityScoreRing";

/* ─── Intelligence types ──────────────────────────────── */

interface WorkContextSummary {
  id: string;
  title: string;
  contextType: string;
  color: string | null;
  durationMins: number;
  confidence: number;
}

interface DashboardNudge {
  message: string;
  nudgeType: string;
  priority: string;
}

interface DashboardIntelligence {
  activeContext: WorkContextSummary | null;
  focusRecommendation: string | null;
  sessionSummary: {
    contextType: string;
    totalDurationMins: number;
    sessionCount: number;
    color: string;
  }[];
  contextSwitches: number;
  switchQuality: string;
  productivityScore: number;
  scoreTrend: number;
  patterns: string[];
  nudges: DashboardNudge[];
  resourceClusters: { resources: string[]; accessCount: number }[];
}

/* ─── Props ───────────────────────────────────────────── */

interface SummaryPanelProps {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  selectedSession?: SessionBlock | null;
  onClose: () => void;
  productivitySummary?: ProductivitySummary | null;
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
  const navigate = useNavigate();

  if (selectedSession) {
    return <SessionDetail session={selectedSession} onClose={onClose} />;
  }
  if (selectedEntry) {
    return <EntryDetail entry={selectedEntry} onClose={onClose} onNavigate={navigate} />;
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

/* ════════════════════════════════════════════════════════════
   Main Day Summary — single unified sidebar
   ════════════════════════════════════════════════════════════ */

function DaySummary({
  summary,
  productivitySummary,
  date,
}: {
  summary: TimelineSummary;
  productivitySummary?: ProductivitySummary | null;
  date: string;
}) {
  const ps = productivitySummary;
  const hasProductivity = ps != null && ps.totalActiveSecs > 0;
  const productivePct = hasProductivity
    ? Math.round((ps.productiveSecs / ps.totalActiveSecs) * 100)
    : 0;

  // Intelligence data
  const { data: intel } = useQuery<DashboardIntelligence>(
    "get_dashboard_intelligence",
    { date, tzOffsetMins: TZ_OFFSET_MINS },
    undefined,
    30_000,
  );

  // Weekly trend for sparkline
  const { data: weeklyData } = useQuery<ProductivitySummary[]>("productivity_weekly", undefined);

  return (
    <div className="w-80 px-4 py-3 flex flex-col gap-3 overflow-y-auto shrink-0">
      {/* ── 1. Score ring (left) + Metrics & active time (right) ── */}
      {hasProductivity && ps.productivityScore != null && (
        <section className="flex items-start gap-3">
          <ProductivityScoreRing score={ps.productivityScore} size={72} />
          <div className="flex-1 min-w-0 flex flex-col gap-1.5 pt-0.5">
            {/* Active time + ratio bar */}
            <div>
              <div className="flex items-center gap-1.5">
                <span className="text-ui-sm font-semibold text-fg tabular-nums">
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
                <span className="text-ui-xs text-fg-dim">active</span>
              </div>
              <div className="flex h-1 rounded-full overflow-hidden bg-control-hover mt-1">
                {ps.productiveSecs > 0 && (
                  <div
                    className="h-full"
                    style={{
                      width: `${(ps.productiveSecs / ps.totalActiveSecs) * 100}%`,
                      backgroundColor: "var(--ds-status-success)",
                    }}
                  />
                )}
                {ps.neutralSecs > 0 && (
                  <div
                    className="h-full"
                    style={{
                      width: `${(ps.neutralSecs / ps.totalActiveSecs) * 100}%`,
                      backgroundColor: "var(--ds-text-secondary)",
                    }}
                  />
                )}
                {ps.distractingSecs > 0 && (
                  <div
                    className="h-full"
                    style={{
                      width: `${(ps.distractingSecs / ps.totalActiveSecs) * 100}%`,
                      backgroundColor: "var(--ds-status-danger)",
                    }}
                  />
                )}
              </div>
              <span className="text-ui-xs text-status-success mt-0.5 block">
                {productivePct}% productive
              </span>
            </div>
            {/* 4 metric bars — compact */}
            {ps.totalActiveSecs > 0 && (
              <div className="flex flex-col gap-0.5">
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
            {/* Deep Work */}
            {ps.deepWorkBlocks > 0 && (
              <div className="flex items-center justify-between text-ui-xs text-fg-dim px-1 mt-1.5">
                <span>
                  {ps.deepWorkBlocks} deep work block{ps.deepWorkBlocks !== 1 ? "s" : ""}
                </span>
                <span>{formatHumanDuration(ps.deepWorkSecs)}</span>
              </div>
            )}

            {/* Recovery Time */}
            {ps.avgRecoverySecs != null && (
              <div className="text-ui-xs text-fg-dim px-1 mt-0.5">
                Avg recovery: {Math.round(ps.avgRecoverySecs)}s
              </div>
            )}
          </div>
        </section>
      )}

      {/* Fallback when no productivity data */}
      {!hasProductivity && (
        <section className="text-sm font-semibold text-fg">
          {formatHumanDuration(summary.totalTrackedSecs)} tracked
        </section>
      )}

      {/* ── 3. LLM suggestion ── */}
      {intel?.focusRecommendation && (
        <p className="text-ui-xs text-fg-secondary italic leading-relaxed">
          {intel.focusRecommendation}
        </p>
      )}

      {/* ── 4. Weekly sparkline ── */}
      {weeklyData && weeklyData.length >= 2 && <WeeklySparkline data={weeklyData} />}

      {/* ── 4b. Patterns ── */}
      {hasProductivity && <PatternsCard />}

      {/* ── 4c. Hourly Heatmap ── */}
      {hasProductivity && <HourlyHeatmap startDate={date} endDate={date} />}

      {/* ── 5. Top Apps — visual bar chart ── */}
      {hasProductivity && ps.topApps.length > 0 && (
        <section>
          <h4 className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-1.5">
            Top Apps
          </h4>
          <TopAppsChart apps={ps.topApps} maxSecs={ps.topApps[0]?.durationSecs ?? 1} />
        </section>
      )}

      {/* ── 6. Insights & Nudges ── */}
      {intel && (intel.patterns.length > 0 || intel.nudges.length > 0) && (
        <section>
          <h4 className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-1.5">
            Insights
          </h4>
          <div className="flex flex-col gap-2">
            {intel.patterns.map((p) => (
              <div key={`pattern-${p}`} className="flex items-start gap-2 text-ui-xs">
                <Brain className="size-3 text-fg-secondary mt-0.5 shrink-0" />
                <span className="text-fg-secondary">{p}</span>
              </div>
            ))}
            {intel.nudges.map((n) => (
              <div
                key={`nudge-${n.nudgeType}-${n.message}`}
                className="flex items-start gap-2 text-ui-xs"
              >
                <Lightbulb className="size-3 text-status-warning mt-0.5 shrink-0" />
                <span className="text-fg-secondary">{n.message}</span>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* ── 7. AI Summary ── */}
      {ps?.aiSummary && (
        <section className="rounded-lg bg-brand/[0.06] border border-brand/15 p-2.5">
          <p className="text-ui-xs text-fg-secondary leading-relaxed">{ps.aiSummary}</p>
        </section>
      )}

      {/* ── 7. Goals ── */}
      <GoalsProgress />
    </div>
  );
}

/* ════════════════════════════════════════════════════════════
   Top Apps visual bar chart
   ════════════════════════════════════════════════════════════ */

function TopAppsChart({
  apps,
  maxSecs,
}: {
  apps: { appName: string; durationSecs: number; category?: string | null }[];
  maxSecs: number;
}) {
  return (
    <div className="flex flex-col gap-1">
      {apps.slice(0, 5).map((app) => {
        const pct = maxSecs > 0 ? (app.durationSecs / maxSecs) * 100 : 0;
        const color = getAppColor(app.appName, app.category ?? null);
        return (
          <div key={app.appName} className="flex items-center gap-1.5">
            <span className="text-ui-xs text-fg-secondary truncate w-16 shrink-0">
              {app.appName}
            </span>
            <div className="flex-1 h-1 rounded-full bg-control-hover overflow-hidden">
              <div
                className="h-full rounded-full"
                style={{
                  width: `${Math.max(pct, 4)}%`,
                  backgroundColor: color,
                  opacity: 0.6 + (pct / 100) * 0.4,
                }}
              />
            </div>
            <span className="text-ui-xs text-fg-dim tabular-nums text-right shrink-0 whitespace-nowrap">
              {formatHumanDuration(app.durationSecs)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/* ════════════════════════════════════════════════════════════
   Weekly sparkline — compact 7-day score trend
   ════════════════════════════════════════════════════════════ */

function WeeklySparkline({ data }: { data: ProductivitySummary[] }) {
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
          stroke="var(--ds-accent)"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx={lastX} cy={lastY} r="2.5" fill="var(--ds-accent)" />
      </svg>
      {changePct !== 0 && (
        <span
          className={`text-ui-xs font-medium shrink-0 ${changePct > 0 ? "text-status-success" : "text-status-danger"}`}
        >
          {changePct > 0 ? "↑" : "↓"}
          {Math.abs(changePct)}%
        </span>
      )}
    </div>
  );
}

/* ════════════════════════════════════════════════════════════
   Session Detail — shown when clicking an activity session block
   ════════════════════════════════════════════════════════════ */

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
    <div className="w-80 px-4 py-3 flex flex-col gap-3 overflow-y-auto shrink-0">
      <div className="flex items-center justify-between">
        <h3 className="text-ui-sm font-semibold text-fg-secondary uppercase tracking-wider">
          Activity Session
        </h3>
        <button type="button" onClick={onClose} className="text-fg-secondary hover:text-fg">
          <X className="size-4" />
        </button>
      </div>

      {/* Session header */}
      <div className="flex items-center gap-2">
        <div className="size-3 rounded-sm" style={{ backgroundColor: session.color }} />
        <span className="text-sm font-medium text-fg">{session.label}</span>
      </div>

      {/* Intelligence description */}
      {matched?.description && (
        <p className="text-ui-sm text-fg-secondary leading-relaxed">{matched.description}</p>
      )}

      {/* Quality score + Category badge row */}
      <div className="flex items-center gap-2 flex-wrap">
        {matched?.qualityScore != null && (
          <div
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-ui-xs font-semibold w-fit"
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
          className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-ui-xs font-medium w-fit"
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

      {/* Intelligence stats */}
      {matched && (
        <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-ui-xs">
          {matched.categoryPurity != null && (
            <>
              <span className="text-fg-dim">Focus purity</span>
              <span className="text-fg-secondary tabular-nums text-right">
                {Math.round(matched.categoryPurity * 100)}%
              </span>
            </>
          )}
          <span className="text-fg-dim">Context switches</span>
          <span className="text-fg-secondary tabular-nums text-right">
            {matched.contextSwitches}
          </span>
          <span className="text-fg-dim">Distractions</span>
          <span className="text-fg-secondary tabular-nums text-right">
            {matched.distractionCount}
          </span>
        </div>
      )}

      {/* Time info */}
      <div className="text-ui-sm text-fg-secondary space-y-1">
        <div>
          {fmt(startH, startM)} – {fmt(endH, endM)}
        </div>
        <div>Duration: {formatHumanDuration(session.duration)}</div>
      </div>

      {/* App breakdown */}
      {session.appBreakdown.length > 0 && (
        <div>
          <h4 className="text-ui-sm font-medium text-fg-secondary mb-2">Apps in this session</h4>
          <div className="flex flex-col gap-2">
            {session.appBreakdown.map((app) => {
              const appCatColor = resolveActivityColor(app.catType, false);
              return (
                <div key={app.app} className="flex items-center gap-2">
                  <span
                    className="w-1.5 h-1.5 rounded-full shrink-0"
                    style={{ backgroundColor: appCatColor }}
                  />
                  <span className="text-ui-sm text-fg-secondary truncate flex-1">{app.app}</span>
                  <span className="text-ui-xs text-fg-dim tabular-nums">
                    {formatHumanDuration(app.dur)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

/* ════════════════════════════════════════════════════════════
   Entry Detail — shown when clicking any other timeline entry
   ════════════════════════════════════════════════════════════ */

function EntryDetail({
  entry,
  onClose,
  onNavigate,
}: {
  entry: TimelineEntry;
  onClose: () => void;
  onNavigate: (path: string) => void;
}) {
  return (
    <div className="w-80 px-4 py-3 flex flex-col gap-3 overflow-y-auto shrink-0">
      <div className="flex items-center justify-between">
        <h3 className="text-ui-sm font-semibold text-fg-secondary uppercase tracking-wider">
          Details
        </h3>
        <button type="button" onClick={onClose} className="text-fg-secondary hover:text-fg">
          <X className="size-4" />
        </button>
      </div>

      <div className="flex items-center gap-2">
        <div className="size-3 rounded-sm" style={{ backgroundColor: entry.color }} />
        <span className="text-sm font-medium text-fg">{entry.title}</span>
      </div>

      {entry.description && <p className="text-ui-sm text-fg-secondary">{entry.description}</p>}

      <div className="text-ui-sm text-fg-secondary space-y-1">
        <div>Started: {new Date(entry.startedAt).toLocaleTimeString()}</div>
        {entry.endedAt && <div>Ended: {new Date(entry.endedAt).toLocaleTimeString()}</div>}
        {entry.durationSecs != null && entry.durationSecs > 0 && (
          <div>Duration: {formatHumanDuration(entry.durationSecs)}</div>
        )}
        <div className="capitalize">Source: {entry.source}</div>
      </div>

      {entry.entityRoute != null && (
        <button
          type="button"
          onClick={() => onNavigate(entry.entityRoute as string)}
          className="flex items-center gap-1.5 text-ui-sm text-brand hover:underline mt-1"
        >
          <ExternalLink className="size-3.5" />
          Open {entry.source}
        </button>
      )}
    </div>
  );
}

/* ── Shared helpers ───────────────────────────────────────── */

function TrendArrow({ value, label }: { value?: number | null; label?: string }) {
  if (value == null || Math.abs(value) < 0.5) return null;
  const isUp = value > 0;
  const pct = Math.round(Math.abs(value));
  return (
    <span
      className={`inline-flex items-center gap-0.5 text-ui-xs font-medium ${isUp ? "text-status-success" : "text-status-danger"}`}
      title={label ? `${isUp ? "+" : "-"}${pct}% ${label}` : undefined}
    >
      {isUp ? "↑" : "↓"}
      {pct > 0 && `${pct}%`}
    </span>
  );
}
