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
    <div className="dashboard__strip-category-bar">
      {segments.map((seg) => (
        <div
          key={seg.key}
          className="dashboard__strip-category-seg"
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
      className="dashboard__strip-mini-score"
      style={{
        background: `conic-gradient(${color} ${clamped * 3.6}deg, rgba(255,255,255,0.06) 0deg)`,
      }}
    >
      <div>
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
    <div className="dashboard__strip-top-apps">
      {apps.map((app) => (
        <div key={app.appName}>
          <span>{app.appName}</span>
          <div>
            <div
              style={{
                width: `${(app.durationSecs / maxDur) * 100}%`,
                backgroundColor: "var(--brand)",
              }}
            />
          </div>
          <span>{formatHumanDuration(app.durationSecs)}</span>
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
    <div className="dashboard__strip">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="dashboard__strip-toggle"
        aria-expanded={expanded}
        aria-controls="productivity-strip-detail"
      >
        <MiniScore score={summary.productivityScore} />
        <div>
          <CategoryBar summary={summary} />
        </div>
        <div className="dashboard__strip-quick-stats">
          <span>
            {formatHumanDuration(summary.totalActiveSecs)} <span>active</span>
          </span>
          <span style={{ color: "var(--success)" }}>
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
          className="dashboard__strip-chevron"
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
        <div id="productivity-strip-detail" className="dashboard__strip-detail">
          <TopAppsMini summary={summary} />
          <div className="dashboard__strip-breakdown">
            <span>
              <span style={{ backgroundColor: "var(--success)" }} />
              {formatHumanDuration(summary.productiveSecs)}
            </span>
            <span>
              <span style={{ backgroundColor: "var(--text-muted-foreground)" }} />
              {formatHumanDuration(summary.neutralSecs)}
            </span>
            <span>
              <span style={{ backgroundColor: "var(--destructive)" }} />
              {formatHumanDuration(summary.distractingSecs)}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
