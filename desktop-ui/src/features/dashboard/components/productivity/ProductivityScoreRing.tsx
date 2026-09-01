import { formatHumanDuration } from "@shared/lib/dates";
import { scoreColor } from "@shared/lib/productivity";
import { useState } from "react";

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
    <div className="flex items-center gap-1.5 text-[9px] font-light">
      <span className="w-[68px] text-muted-foreground text-right shrink-0 truncate">{label}</span>
      <div className="flex-1 h-1 rounded-full bg-muted overflow-hidden">
        <div
          className="h-full rounded-full bg-brand/60 transition-[width]"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="w-7 text-dim tabular-nums shrink-0">{pct}%</span>
    </div>
  );
}

function scoreLabel(score: number): string {
  if (score >= 80) return "Excellent";
  if (score >= 60) return "Good";
  if (score >= 40) return "Fair";
  if (score > 0) return "Low";
  return "\u2014";
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
      {/* biome-ignore lint/a11y/noStaticElementInteractions: hover tooltip container for score details */}
      <div
        className="relative"
        style={{ width: size, height: size }}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        {/* Glow effect behind the ring */}
        <div
          className="absolute inset-2 rounded-full transition-opacity duration-700"
          style={{
            background: `radial-gradient(circle, ${color}15 0%, transparent 70%)`,
            opacity: score > 0 ? 1 : 0,
          }}
        />

        <svg width={size} height={size} className="-rotate-90" aria-hidden="true">
          {/* Background track */}
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke="var(--surface-raised)"
            strokeWidth={strokeWidth}
          />
          {/* Progress arc */}
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
            className="transition-[stroke-dashoffset] duration-1000"
            style={{
              filter: `drop-shadow(0 0 4px ${color}66)`,
            }}
          />
        </svg>

        {/* Center content */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-[26px] font-light tabular-nums leading-none" style={{ color }}>
            {Math.round(score)}
          </span>
          <span className="text-[9px] font-light text-dim mt-0.5">/100</span>
        </div>

        {/* Hover tooltip */}
        {hovered && summary && summary.totalActiveSecs > 0 && (
          <div className="absolute left-1/2 -translate-x-1/2 top-full mt-2 z-50 glass-panel rounded-lg px-3 py-2 shadow-lg min-w-[160px]">
            <div className="flex flex-col gap-1.5 text-2xs">
              {focusRatio != null && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Focus time</span>
                  <span className="text-foreground tabular-nums">
                    {focusRatio}% ({formatHumanDuration(summary.productiveSecs)})
                  </span>
                </div>
              )}
              <div className="flex justify-between">
                <span className="text-muted-foreground">Context switches</span>
                <span className="text-foreground tabular-nums">{summary.contextSwitches}</span>
              </div>
              {qualityAvg != null && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Session quality</span>
                  <span className="text-foreground tabular-nums">{qualityAvg}%</span>
                </div>
              )}
              {distractionRatio != null && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Distraction</span>
                  <span className="text-destructive tabular-nums">{distractionRatio}%</span>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Label below */}
      <span className="text-2xs font-medium tracking-wide uppercase" style={{ color }}>
        {scoreLabel(score)}
      </span>

      {/* Score bars rendered externally via <ScoreBar /> */}
    </div>
  );
}
