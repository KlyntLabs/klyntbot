import { CheckCircle, DollarSign, ExternalLink, FileText, ListTodo, X } from "lucide-react";
import { useNavigate } from "react-router";
import { formatHumanDuration, todayISO } from "../../lib/dates";
import { useQuery } from "../../hooks/useQuery";
import type {
  ProductivitySummary,
  TimelineEntry,
  TimelineSummary,
} from "../../lib/types";
import { AiSummaryCard } from "../productivity/AiSummaryCard";
import { DistractionBanner } from "../productivity/DistractionBanner";
import { FocusSessionsList } from "../productivity/FocusSessionsList";
import { GoalsProgress } from "../productivity/GoalsProgress";
import { InsightCardList } from "../productivity/InsightCardList";
import { ProductivityScoreRing } from "../productivity/ProductivityScoreRing";
import { resolveActivityColor, resolveCategoryLabel } from "../productivity/shared";
import { WorkHoursCard } from "../productivity/WorkHoursCard";
import type { SessionBlock } from "./ActivityTrack";

interface SummaryPanelProps {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  selectedSession?: SessionBlock | null;
  onClose: () => void;
  productivitySummary?: ProductivitySummary | null;
}

export function SummaryPanel({
  summary,
  selectedEntry,
  selectedSession,
  onClose,
  productivitySummary,
}: SummaryPanelProps) {
  const navigate = useNavigate();

  if (selectedSession) {
    return <SessionDetail session={selectedSession} onClose={onClose} />;
  }
  if (selectedEntry) {
    return <EntryDetail entry={selectedEntry} onClose={onClose} onNavigate={navigate} />;
  }
  if (!summary) return null;
  return <DefaultSummary summary={summary} productivitySummary={productivitySummary} />;
}

const SOURCE_ORDER: Record<string, number> = {
  focus: 0,
  task: 1,
  todo: 2,
  note: 3,
  finance: 4,
};

function DefaultSummary({
  summary,
  productivitySummary,
}: {
  summary: TimelineSummary;
  productivitySummary?: ProductivitySummary | null;
}) {
  const sortedBreakdown = [...summary.sourceBreakdown].sort(
    (a, b) => (SOURCE_ORDER[a.source] ?? 9) - (SOURCE_ORDER[b.source] ?? 9),
  );

  const ps = productivitySummary;
  const hasProductivity = ps != null && ps.totalActiveSecs > 0;
  const productivePct = hasProductivity
    ? Math.round((ps.productiveSecs / ps.totalActiveSecs) * 100)
    : 0;

  return (
    <div className="w-72 glass-card p-4 flex flex-col gap-4 overflow-y-auto">
      <h3 className="text-xs font-semibold text-muted uppercase tracking-wider">Summary</h3>

      {/* Productivity score ring */}
      {hasProductivity && ps.productivityScore != null && (
        <div className="flex justify-center py-1">
          <ProductivityScoreRing
            score={ps.productivityScore}
            size={100}
            summary={{
              productiveSecs: ps.productiveSecs,
              neutralSecs: ps.neutralSecs,
              distractingSecs: ps.distractingSecs,
              totalActiveSecs: ps.totalActiveSecs,
              avgSessionQuality: ps.avgSessionQuality,
              focusSessionsCount: ps.focusSessionsCount,
              contextSwitches: ps.contextSwitches,
            }}
          />
        </div>
      )}

      {/* Activity ratio bar */}
      {hasProductivity && (
        <div className="p-2.5 rounded-lg border border-success/20 bg-success/[0.06]">
          <div className="flex items-center justify-between mb-2">
            <div className="text-sm font-semibold text-primary tabular-nums">
              {formatHumanDuration(ps.totalActiveSecs)}
            </div>
            <div className="text-[10px] text-muted">active time</div>
          </div>
          <div className="flex h-1.5 rounded-full overflow-hidden bg-white/[0.06]">
            {ps.productiveSecs > 0 && (
              <div
                className="h-full"
                style={{
                  width: `${(ps.productiveSecs / ps.totalActiveSecs) * 100}%`,
                  backgroundColor: "var(--success)",
                }}
              />
            )}
            {ps.neutralSecs > 0 && (
              <div
                className="h-full"
                style={{
                  width: `${(ps.neutralSecs / ps.totalActiveSecs) * 100}%`,
                  backgroundColor: "var(--text-muted)",
                }}
              />
            )}
            {ps.distractingSecs > 0 && (
              <div
                className="h-full"
                style={{
                  width: `${(ps.distractingSecs / ps.totalActiveSecs) * 100}%`,
                  backgroundColor: "var(--destructive)",
                }}
              />
            )}
          </div>
          <div className="flex items-center justify-between mt-1.5 text-[9px]">
            <span className="text-success">{productivePct}% productive</span>
            {ps.distractingSecs > 0 && (
              <span className="text-destructive">
                {Math.round((ps.distractingSecs / ps.totalActiveSecs) * 100)}% distracting
              </span>
            )}
          </div>
        </div>
      )}

      {/* 7-day trend */}
      {hasProductivity && <WeeklyTrend />}

      {/* Focus — primary stat */}
      <div className="p-2 rounded-lg bg-timeline-focus/10 border border-timeline-focus/20">
        <div className="text-lg font-semibold text-primary">
          {formatHumanDuration(summary.focusSecs)}
        </div>
        <div className="text-[10px] text-muted">Focus time</div>
        {summary.totalTrackedSecs > 0 && (
          <div className="text-[10px] text-brand mt-0.5">
            {Math.round((summary.focusSecs / summary.totalTrackedSecs) * 100)}% focus ratio
          </div>
        )}
      </div>

      {/* Total tracked */}
      <div>
        <div className="text-sm font-semibold text-primary">
          {formatHumanDuration(summary.totalTrackedSecs)}
        </div>
        <div className="text-[10px] text-muted">total tracked</div>
      </div>

      {/* Quick stats */}
      <div className="grid grid-cols-2 gap-2">
        <Stat
          icon={<CheckCircle className="w-3.5 h-3.5" />}
          label="Completed"
          value={String(summary.tasksCompleted)}
        />
        <Stat
          icon={<ListTodo className="w-3.5 h-3.5" />}
          label="Created"
          value={String(summary.tasksCreated)}
        />
        <Stat
          icon={<FileText className="w-3.5 h-3.5" />}
          label="Notes"
          value={String(summary.notesTouched)}
        />
        <Stat
          icon={<DollarSign className="w-3.5 h-3.5" />}
          label="Transactions"
          value={String(summary.transactionsCount)}
        />
      </div>

      {/* Top apps */}
      {hasProductivity && ps.topApps.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted mb-2">Top Apps</h4>
          <div className="flex flex-col gap-1.5">
            {ps.topApps.slice(0, 5).map((app) => (
              <div key={app.appName} className="flex items-center gap-2 text-xs">
                <span className="text-secondary truncate flex-1">{app.appName}</span>
                <span className="text-dim tabular-nums text-[10px]">
                  {formatHumanDuration(app.durationSecs)}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Source breakdown */}
      {sortedBreakdown.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted mb-2">Breakdown</h4>
          <div className="flex flex-col gap-1.5">
            {sortedBreakdown.map((s) => (
              <div key={s.source} className="flex items-center justify-between text-xs">
                <span className="text-secondary capitalize">{s.source}</span>
                <span className="text-muted">{s.count} items</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Distraction banner */}
      {hasProductivity && <DistractionBanner summary={ps} />}

      {/* Work hours progress */}
      {hasProductivity && <WorkHoursCard totalActiveSecs={ps.totalActiveSecs} />}

      {/* Focus sessions */}
      <FocusSessionsList date={todayISO()} />

      {/* Goals */}
      <GoalsProgress />

      {/* Insights */}
      <InsightCardList date={todayISO()} />

      {/* AI Summary */}
      {ps?.aiSummary && <AiSummaryCard summary={ps.aiSummary} />}
    </div>
  );
}

/** Session detail panel — shown when clicking an activity session block */
function SessionDetail({ session, onClose }: { session: SessionBlock; onClose: () => void }) {
  const startH = Math.floor(session.startMin / 60);
  const startM = Math.floor(session.startMin % 60);
  const endH = Math.floor(session.endMin / 60);
  const endM = Math.floor(session.endMin % 60);
  const fmt = (h: number, m: number) =>
    `${h % 12 || 12}:${String(m).padStart(2, "0")} ${h < 12 ? "AM" : "PM"}`;

  const categoryLabel = resolveCategoryLabel(session.dominantCategory);
  const categoryColor = resolveActivityColor(session.dominantCategory, false);

  return (
    <div className="w-72 glass-card p-4 flex flex-col gap-3 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-muted uppercase tracking-wider">
          Activity Session
        </h3>
        <button type="button" onClick={onClose} className="text-muted hover:text-secondary">
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Session header */}
      <div className="flex items-center gap-2">
        <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: session.color }} />
        <span className="text-sm font-medium text-primary">{session.label}</span>
      </div>

      {/* Category badge */}
      <div
        className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-medium w-fit"
        style={{
          backgroundColor: `color-mix(in oklch, ${categoryColor} 15%, transparent)`,
          color: categoryColor,
          border: `1px solid color-mix(in oklch, ${categoryColor} 25%, transparent)`,
        }}
      >
        <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: categoryColor }} />
        {categoryLabel}
      </div>

      {/* Time info */}
      <div className="text-xs text-muted space-y-1">
        <div>
          {fmt(startH, startM)} – {fmt(endH, endM)}
        </div>
        <div>Duration: {formatHumanDuration(session.duration)}</div>
      </div>

      {/* App breakdown */}
      {session.appBreakdown.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted mb-2">Apps in this session</h4>
          <div className="flex flex-col gap-2">
            {session.appBreakdown.map((app) => {
              const appCatColor = resolveActivityColor(app.catType, false);
              return (
                <div key={app.app} className="flex items-center gap-2">
                  <span
                    className="w-1.5 h-1.5 rounded-full shrink-0"
                    style={{ backgroundColor: appCatColor }}
                  />
                  <span className="text-xs text-secondary truncate flex-1">{app.app}</span>
                  <span className="text-[10px] text-dim tabular-nums">
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
    <div className="w-72 glass-card p-4 flex flex-col gap-3 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-muted uppercase tracking-wider">Details</h3>
        <button type="button" onClick={onClose} className="text-muted hover:text-secondary">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex items-center gap-2">
        <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: entry.color }} />
        <span className="text-sm font-medium text-primary">{entry.title}</span>
      </div>

      {entry.description && <p className="text-xs text-muted">{entry.description}</p>}

      <div className="text-xs text-muted space-y-1">
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
          className="flex items-center gap-1.5 text-xs text-brand hover:underline mt-1"
        >
          <ExternalLink className="w-3.5 h-3.5" />
          Open {entry.source}
        </button>
      )}
    </div>
  );
}

function Stat({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="glass-card p-2 flex flex-col gap-0.5">
      <div className="flex items-center gap-1 text-muted">
        {icon}
        <span className="text-[10px]">{label}</span>
      </div>
      <div className="text-sm font-semibold text-primary">{value}</div>
    </div>
  );
}

function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return null;
  const w = 120;
  const h = 28;
  const pad = 2;
  const max = Math.max(...values, 1);
  const min = Math.min(...values, 0);
  const range = max - min || 1;
  const points = values
    .map((v, i) => {
      const x = pad + (i / (values.length - 1)) * (w - pad * 2);
      const y = h - pad - ((v - min) / range) * (h - pad * 2);
      return `${x},${y}`;
    })
    .join(" ");

  return (
    <svg width={w} height={h} className="block">
      <polyline
        points={points}
        fill="none"
        stroke="var(--brand)"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* Dot on last point */}
      {values.length > 0 && (() => {
        const lastX = pad + ((values.length - 1) / (values.length - 1)) * (w - pad * 2);
        const lastY = h - pad - ((values[values.length - 1] - min) / range) * (h - pad * 2);
        return <circle cx={lastX} cy={lastY} r="2.5" fill="var(--brand)" />;
      })()}
    </svg>
  );
}

function WeeklyTrend() {
  const { data: weeklyData } = useQuery<ProductivitySummary[]>("productivity_weekly", undefined);

  if (!weeklyData || weeklyData.length < 2) return null;

  const scores = weeklyData.map((d) => d.productivityScore ?? 0);

  // Compare this week's avg vs prior week's avg
  const halfLen = Math.floor(scores.length / 2);
  const recentScores = scores.slice(halfLen);
  const olderScores = scores.slice(0, halfLen);
  const recentAvg =
    recentScores.length > 0 ? recentScores.reduce((a, b) => a + b, 0) / recentScores.length : 0;
  const olderAvg =
    olderScores.length > 0 ? olderScores.reduce((a, b) => a + b, 0) / olderScores.length : 0;

  const changePct = olderAvg > 0 ? Math.round(((recentAvg - olderAvg) / olderAvg) * 100) : 0;
  const isUp = changePct >= 0;

  return (
    <div className="p-2.5 rounded-lg border border-brand/15 bg-brand/[0.04]">
      <h4 className="text-[10px] font-medium text-muted mb-1.5">7-day trend</h4>
      <Sparkline values={scores} />
      {changePct !== 0 && (
        <div className="flex items-center gap-1 mt-1.5">
          <span className={`text-[10px] font-medium ${isUp ? "text-success" : "text-destructive"}`}>
            {isUp ? "\u2191" : "\u2193"} {Math.abs(changePct)}%
          </span>
          <span className="text-[9px] text-dim">vs last week</span>
        </div>
      )}
    </div>
  );
}
