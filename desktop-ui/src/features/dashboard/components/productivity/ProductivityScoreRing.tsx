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
    <div className="flex items-center gap-1.5 text-ui-2xs font-light">
      <span className="w-[68px] text-ds-text-subtle text-right shrink-0">{label}</span>
      <div className="flex-1 h-1 rounded-full bg-surface-control overflow-hidden">
        <div className="h-full bg-[color-mix(in_srgb,var(--brand)_60%,transparent)]" style={{ width: `${pct}%` }} />
      </div>
      <span className="w-7 text-right text-ds-text-subtle tabular-nums">{pct}</span>
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

export function ProductivityScoreRing({ score, size = 110, summary }: ProductivityScoreRingProps) {
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
    <div className="flex flex-col items-center gap-2">
      <div
        className="relative"
        style={{ width: size, height: size }}
        role="img"
        aria-label={`Productivity score ${Math.round(score)} out of 100`}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onFocus={() => setHovered(true)}
        onBlur={() => setHovered(false)}
      >
        <div
          className="absolute rounded-full transition-opacity duration-700 ease-out pointer-events-none"
          style={{
            inset: 8,
            background: `radial-gradient(circle, ${color}15 0%, transparent 70%)`,
            opacity: score > 0 ? 1 : 0,
          }}
        />

        <svg width={size} height={size} className="-rotate-90" aria-hidden="true">
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

        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span style={{ color }} className="text-ui-display-sm font-light tabular-nums leading-none">{Math.round(score)}</span>
          <span className="text-ui-2xs font-light text-ds-text-subtle mt-0.5">/100</span>
        </div>

        {hovered && summary && summary.totalActiveSecs > 0 && (
          <div className="absolute left-1/2 top-full mt-2 -translate-x-1/2 z-50 bg-surface-popover border border-ds-border-subtle shadow-ds-popover rounded-lg px-3 py-2 min-w-[160px] flex flex-col gap-1.5 text-ui-2xs">
            {focusRatio != null && (
              <div className="flex justify-between gap-2">
                <span>Focus time</span>
                <span>
                  {focusRatio}% ({formatHumanDuration(summary.productiveSecs)})
                </span>
              </div>
            )}
            <div className="flex justify-between gap-2">
              <span>Context switches</span>
              <span>{summary.contextSwitches}</span>
            </div>
            {qualityAvg != null && (
              <div className="flex justify-between gap-2">
                <span>Session quality</span>
                <span>{qualityAvg}%</span>
              </div>
            )}
            {distractionRatio != null && (
              <div className="flex justify-between gap-2">
                <span>Distraction</span>
                <span>{distractionRatio}%</span>
              </div>
            )}
          </div>
        )}
      </div>

      <span className="text-ui-2xs font-medium tracking-wider uppercase" style={{ color }}>
        {scoreLabel(score)}
      </span>
    </div>
  );
}
