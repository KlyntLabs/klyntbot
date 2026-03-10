/**
 * Compact horizontal productivity bar for the dashboard day view.
 * Shows merged session blocks with auto-zoom + a mini score gauge.
 */

import { useState } from "react";
import { formatHumanDuration } from "../../lib/dates";
import type { ProductivitySummary } from "../../lib/types";
import { scoreColor } from "../productivity/shared";

interface ProductivityStripProps {
  summary: ProductivitySummary | null;
}

/** Category ratio bar — shows productive / neutral / distracting proportions */
function CategoryBar({ summary }: { summary: ProductivitySummary }) {
  const total = summary.productiveSecs + summary.neutralSecs + summary.distractingSecs;
  if (total === 0) return null;

  const segments = [
    { key: "productive", secs: summary.productiveSecs, color: "var(--success)" },
    { key: "neutral", secs: summary.neutralSecs, color: "var(--text-muted)" },
    { key: "distracting", secs: summary.distractingSecs, color: "var(--destructive)" },
  ].filter((s) => s.secs > 0);

  return (
    <div className="flex h-1.5 rounded-full overflow-hidden bg-white/[0.06]">
      {segments.map((seg) => (
        <div
          key={seg.key}
          className="h-full first:rounded-l-full last:rounded-r-full"
          style={{
            width: `${(seg.secs / total) * 100}%`,
            backgroundColor: seg.color,
          }}
        />
      ))}
    </div>
  );
}

/** Mini score arc — compact score indicator */
function MiniScore({ score }: { score: number | null }) {
  if (score == null) return null;
  const clamped = Math.max(0, Math.min(100, score));
  const color = scoreColor(clamped);

  return (
    <div
      className="w-6 h-6 rounded-full flex items-center justify-center text-[9px] font-semibold tabular-nums shrink-0"
      style={{
        background: `conic-gradient(${color} ${clamped * 3.6}deg, rgba(255,255,255,0.06) 0deg)`,
      }}
    >
      <div className="w-[18px] h-[18px] rounded-full bg-[rgba(0,0,0,0.85)] flex items-center justify-center">
        <span style={{ color }}>{Math.round(clamped)}</span>
      </div>
    </div>
  );
}

/** Top apps mini list — shows top 3 apps with tiny bars */
function TopAppsMini({ summary }: { summary: ProductivitySummary }) {
  const apps = summary.topApps.slice(0, 3);
  if (apps.length === 0) return null;
  const maxDur = apps[0]?.durationSecs ?? 1;

  return (
    <div className="flex flex-col gap-1">
      {apps.map((app) => (
        <div key={app.appName} className="flex items-center gap-2">
          <span className="text-[9px] text-muted truncate w-14">{app.appName}</span>
          <div className="flex-1 h-1 rounded-full bg-white/[0.06] overflow-hidden">
            <div
              className="h-full rounded-full"
              style={{
                width: `${(app.durationSecs / maxDur) * 100}%`,
                backgroundColor: "var(--brand)",
              }}
            />
          </div>
          <span className="text-[9px] text-dim tabular-nums w-6 text-right">
            {formatHumanDuration(app.durationSecs)}
          </span>
        </div>
      ))}
    </div>
  );
}

export function ProductivityStrip({ summary }: ProductivityStripProps) {
  const [expanded, setExpanded] = useState(false);

  if (!summary || summary.totalActiveSecs === 0) return null;

  const productivePct = Math.round((summary.productiveSecs / summary.totalActiveSecs) * 100);

  return (
    <div className="border-b border-border bg-white/[0.02]">
      {/* Compact bar — always visible */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="w-full px-3 py-2 flex items-center gap-3 hover:bg-white/[0.03] transition-colors"
      >
        <MiniScore score={summary.productivityScore} />

        <div className="flex-1 min-w-0">
          <CategoryBar summary={summary} />
        </div>

        {/* Quick stats */}
        <div className="flex items-center gap-3 text-[10px] tabular-nums shrink-0">
          <span className="text-secondary">
            {formatHumanDuration(summary.totalActiveSecs)}
            <span className="text-dim"> active</span>
          </span>
          <span style={{ color: "var(--success)" }}>
            {productivePct}%<span className="text-dim"> productive</span>
          </span>
          {summary.focusSessionsCount > 0 && (
            <span className="text-muted">
              {summary.focusSessionsCount}
              <span className="text-dim"> sessions</span>
            </span>
          )}
        </div>

        <svg
          aria-hidden="true"
          className="w-3 h-3 text-dim transition-transform"
          style={{ transform: expanded ? "rotate(180deg)" : undefined }}
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        >
          <path d="M3 5l3 3 3-3" />
        </svg>
      </button>

      {/* Expanded detail — top apps + breakdown */}
      {expanded && (
        <div className="px-3 pb-2.5 pt-0.5 flex gap-6 animate-in slide-in-from-top-1 duration-150">
          <div className="flex-1 min-w-0">
            <TopAppsMini summary={summary} />
          </div>
          <div className="flex items-center gap-4 text-[9px] shrink-0">
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-success" />
              <span className="text-muted">{formatHumanDuration(summary.productiveSecs)}</span>
            </span>
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-[var(--text-muted)]" />
              <span className="text-muted">{formatHumanDuration(summary.neutralSecs)}</span>
            </span>
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-destructive" />
              <span className="text-muted">{formatHumanDuration(summary.distractingSecs)}</span>
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
