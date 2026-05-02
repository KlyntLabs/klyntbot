import { useState } from "react";
import { formatHumanDuration } from "@/utils/dashboardDates";
import { scoreColor } from "../../lib/productivity";

interface ProductivityScoreRingProps {
  score: number;
  size?: number;
  summary?: {
    productiveSecs: number;
    neutralSecs: number;
    distractingSecs: number;
    totalActiveSecs: number;
    avgSessionQuality: number | null;
    focusSessionsCount: number;
    contextSwitches: number;
  } | null;
}

export function ScoreBar({ label, value }: { label: string; value: number }) {
  const pct = Math.round(Math.min(Math.max(value, 0), 1) * 100);
  return (
    <div className="dashboard__summary-score-bar">
      <span className="dashboard__summary-score-label">{label}</span>
      <div className="dashboard__summary-score-track">
        <div className="dashboard__summary-score-fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="dashboard__summary-score-value">{pct}</span>
    </div>
  );
}

function scoreLabel(score: number): string {
  if (score >= 80) return "Excellent";
  if (score >= 60) return "Good";
  if (score >= 40) return "Fair";
  if (score > 0) return "Low";
  return "—";
}

export function ProductivityScoreRing({
  score,
  size = 110,
  summary,
}: ProductivityScoreRingProps) {
  const [hovered, setHovered] = useState(false);
  const strokeWidth = 7;
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const progress = Math.min(score / 100, 1);
  const offset = circumference * (1 - progress);
  const center = size / 2;
  const color = scoreColor(score);

  const focusRatio =
    summary && summary.totalActiveSecs > 0
      ? Math.round((summary.productiveSecs / summary.totalActiveSecs) * 100)
      : null;
  const distractionRatio =
    summary && summary.totalActiveSecs > 0
      ? Math.round((summary.distractingSecs / summary.totalActiveSecs) * 100)
      : null;
  const qualityAvg =
    summary?.avgSessionQuality != null ? Math.round(summary.avgSessionQuality * 100) : null;

  return (
    <div className="dashboard__score-ring">
      <div
        className="dashboard__score-ring-track"
        style={{ width: size, height: size }}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        <div
          className="dashboard__score-ring-glow"
          style={{
            background: `radial-gradient(circle, ${color}15 0%, transparent 70%)`,
            opacity: score > 0 ? 1 : 0,
          }}
        />

        <svg width={size} height={size} className="dashboard__score-ring-svg" aria-hidden="true">
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke="var(--surface-raised)"
            strokeWidth={strokeWidth}
          />
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={offset}
            style={{ filter: `drop-shadow(0 0 4px ${color}66)` }}
          />
        </svg>

        <div className="dashboard__score-ring-value">
          <span style={{ color }}>{Math.round(score)}</span>
          <span className="dashboard__score-ring-suffix">/100</span>
        </div>

        {hovered && summary && summary.totalActiveSecs > 0 && (
          <div className="dashboard__score-ring-tooltip">
            {focusRatio != null && (
              <div className="dashboard__score-ring-tooltip-row">
                <span>Focus time</span>
                <span>
                  {focusRatio}% ({formatHumanDuration(summary.productiveSecs)})
                </span>
              </div>
            )}
            <div className="dashboard__score-ring-tooltip-row">
              <span>Context switches</span>
              <span>{summary.contextSwitches}</span>
            </div>
            {qualityAvg != null && (
              <div className="dashboard__score-ring-tooltip-row">
                <span>Session quality</span>
                <span>{qualityAvg}%</span>
              </div>
            )}
            {distractionRatio != null && (
              <div className="dashboard__score-ring-tooltip-row">
                <span>Distraction</span>
                <span>{distractionRatio}%</span>
              </div>
            )}
          </div>
        )}
      </div>

      <span className="dashboard__score-ring-label" style={{ color }}>
        {scoreLabel(score)}
      </span>
    </div>
  );
}
