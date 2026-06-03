import { useState } from "react";
import type { ProductivitySummaryResponse } from "@/bindings";
import { formatHumanDuration } from "@/utils/dashboardDates";
import { scoreColor } from "../lib/productivity";

interface ProductivityStripProps {
  summary: ProductivitySummaryResponse | null;
}

function CategoryBar({ summary }: { summary: ProductivitySummaryResponse }) {
  const total = summary.productiveSecs + summary.neutralSecs + summary.distractingSecs;
  if (total === 0) return null;
  const segments = [
    { key: "productive", secs: summary.productiveSecs, color: "var(--success)" },
    { key: "neutral", secs: summary.neutralSecs, color: "var(--text-muted-foreground)" },
    { key: "distracting", secs: summary.distractingSecs, color: "var(--destructive)" },
  ].filter((s) => s.secs > 0);

  return (
    <div className="flex h-1.5 rounded-full overflow-hidden bg-surface-control">
      {segments.map((seg) => (
        <div
          key={seg.key}
          className="h-full"
          style={{
            width: `${(seg.secs / total) * 100}%`,
            backgroundColor: seg.color,
          }}
        />
      ))}
    </div>
  );
}

function MiniScore({ score }: { score: number | null }) {
  if (score == null) return null;
  const clamped = Math.max(0, Math.min(100, score));
  const color = scoreColor(clamped);
  return (
    <div
      className="w-6 h-6 rounded-full flex items-center justify-center text-ui-3xs font-semibold tabular-nums shrink-0"
      style={{
        background: `conic-gradient(${color} ${clamped * 3.6}deg, rgba(255,255,255,0.06) 0deg)`,
      }}
    >
      <div className="w-[18px] h-[18px] rounded-full bg-[var(--ds-popover-bg)] flex items-center justify-center">
        <span style={{ color }}>{Math.round(clamped)}</span>
      </div>
    </div>
  );
}

function TopAppsMini({ summary }: { summary: ProductivitySummaryResponse }) {
  const apps = summary.topApps.slice(0, 3);
  if (apps.length === 0) return null;
  const maxDur = apps[0]?.durationSecs ?? 1;

  return (
    <div className="flex-1 min-w-0 flex flex-col gap-1">
      {apps.map((app) => (
        <div className="flex items-center gap-2" key={app.appName}>
          <span className="text-ui-3xs text-ds-text-subtle w-14 whitespace-nowrap overflow-hidden text-ellipsis">
            {app.appName}
          </span>
          <div className="flex-1 h-1 rounded-full bg-surface-control overflow-hidden">
            <div
              className="h-full rounded-full"
              style={{
                width: `${(app.durationSecs / maxDur) * 100}%`,
                backgroundColor: "var(--brand)",
              }}
            />
          </div>
          <span className="text-ui-3xs text-ds-text-subtle tabular-nums w-6 text-right">
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
    <div className="border-b border-ds-border-subtle bg-surface-card-strong">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="w-full px-3 py-2 flex items-center gap-3 bg-transparent border-none cursor-pointer hover:bg-surface-card-strong"
        aria-expanded={expanded}
        aria-controls="productivity-strip-detail"
      >
        <MiniScore score={summary.productivityScore} />
        <div className="flex-1">
          <CategoryBar summary={summary} />
        </div>
        <div className="flex items-center gap-3 text-ui-2xs tabular-nums shrink-0 text-ds-text-subtle">
          <span>
            {formatHumanDuration(summary.totalActiveSecs)} <span>active</span>
          </span>
          <span className="text-success">
            {productivePct}% <span>productive</span>
          </span>
          {summary.focusSessionsCount > 0 && (
            <span>
              {summary.focusSessionsCount} <span>sessions</span>
            </span>
          )}
        </div>
        <svg
          aria-hidden="true"
          className="w-3 h-3 text-ds-text-subtle transition-transform duration-200"
          style={{ transform: expanded ? "rotate(180deg)" : undefined }}
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        >
          <path d="M3 5l3 3 3-3" />
        </svg>
      </button>

      {expanded && (
        <div
          id="productivity-strip-detail"
          className="px-3 pb-2.5 flex gap-6"
          style={{ animation: "fade-in 0.15s ease-out" }}
        >
          <TopAppsMini summary={summary} />
          <div className="flex items-center gap-4 text-ui-3xs shrink-0">
            <span className="inline-flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: "var(--success)" }} />
              {formatHumanDuration(summary.productiveSecs)}
            </span>
            <span className="inline-flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: "var(--text-muted-foreground)" }} />
              {formatHumanDuration(summary.neutralSecs)}
            </span>
            <span className="inline-flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: "var(--destructive)" }} />
              {formatHumanDuration(summary.distractingSecs)}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
