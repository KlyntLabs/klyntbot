/**
 * Compact horizontal productivity bar for the dashboard day view.
 * Shows merged session blocks with auto-zoom + a mini score gauge.
 */

import { formatHumanDuration } from "@shared/lib/dates";
import { scoreColor } from "@shared/lib/productivity";
import type { ProductivitySummary } from "@shared/types";
import { useState } from "react";

interface ProductivityStripProps {
  summary: ProductivitySummary | null;
}

/** Category ratio bar — shows productive / neutral / distracting proportions */
function CategoryBar({ summary }: { summary: ProductivitySummary }) {
  const total = summary.productiveSecs + summary.neutralSecs + summary.distractingSecs;
  if (total === 0) return null;

  const segments = [
    { key: "productive", secs: summary.productiveSecs, color: "var(--ds-status-success)" },
    { key: "neutral", secs: summary.neutralSecs, color: "var(--ds-text-secondary)" },
    { key: "distracting", secs: summary.distractingSecs, color: "var(--ds-status-danger)" },
  ].filter((s) => s.secs > 0);

  return (
    <div className="flex h-1.5 rounded-full overflow-hidden bg-control-hover">
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
      className="size-6 rounded-full flex items-center justify-center text-[9px] font-semibold tabular-nums shrink-0"
      style={{
        background: `conic-gradient(${color} ${clamped * 3.6}deg, rgba(255,255,255,0.06) 0deg)`,
      }}
    >
      <div className="w-[18px] h-[18px] rounded-full bg-glass-strong flex items-center justify-center">
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
          <span className="text-[9px] text-fg-secondary truncate w-14">{app.appName}</span>
          <div className="flex-1 h-1 rounded-full bg-control-hover overflow-hidden">
            <div
              className="h-full rounded-full"
              style={{
                width: `${(app.durationSecs / maxDur) * 100}%`,
                backgroundColor: "var(--ds-accent)",
              }}
            />
          </div>
          <span className="text-[9px] text-fg-dim tabular-nums w-6 text-right">
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
    <div className="border-b border-separator bg-bg-elevated">
      {/* Compact bar — always visible */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="w-full px-3 py-2 flex items-center gap-3 hover:bg-bg-elevated transition-colors"
      >
        <MiniScore score={summary.productivityScore} />

        <div className="flex-1 min-w-0">
          <CategoryBar summary={summary} />
        </div>

        {/* Quick stats */}
        <div className="flex items-center gap-3 text-ui-xs tabular-nums shrink-0">
          <span className="text-fg-secondary">
            {formatHumanDuration(summary.totalActiveSecs)}
            <span className="text-fg-dim"> active</span>
          </span>
          <span style={{ color: "var(--ds-status-success)" }}>
            {productivePct}%<span className="text-fg-dim"> productive</span>
          </span>
          {summary.focusSessionsCount > 0 && (
            <span className="text-fg-secondary">
              {summary.focusSessionsCount}
              <span className="text-fg-dim"> sessions</span>
            </span>
          )}
        </div>

        <svg
          aria-hidden="true"
          className="size-3 text-fg-dim transition-transform"
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
              <span className="w-1.5 h-1.5 rounded-full bg-status-success" />
              <span className="text-fg-secondary">
                {formatHumanDuration(summary.productiveSecs)}
              </span>
            </span>
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-[var(--ds-text-secondary)]" />
              <span className="text-fg-secondary">
                {formatHumanDuration(summary.neutralSecs)}
              </span>
            </span>
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-status-danger" />
              <span className="text-fg-secondary">
                {formatHumanDuration(summary.distractingSecs)}
              </span>
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
