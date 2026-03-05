import { scoreColor } from "./shared";

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

function ScoreBar({ label, value }: { label: string; value: number }) {
  const pct = Math.round(Math.min(Math.max(value, 0), 1) * 100);
  return (
    <div className="flex items-center gap-2 text-[10px] font-light">
      <span className="w-20 text-muted text-right">{label}</span>
      <div className="flex-1 h-1 rounded-full bg-surface-raised overflow-hidden">
        <div
          className="h-full rounded-full bg-brand/60 transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="w-8 text-dim tabular-nums">{pct}%</span>
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
  const strokeWidth = 7;
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const progress = Math.min(score / 100, 1);
  const offset = circumference * (1 - progress);
  const center = size / 2;
  const color = scoreColor(score);

  return (
    <div className="flex flex-col items-center gap-2">
      <div className="relative" style={{ width: size, height: size }}>
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
            className="transition-all duration-1000"
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
      </div>

      {/* Label below */}
      <span className="text-[10px] font-medium tracking-wide uppercase" style={{ color }}>
        {scoreLabel(score)}
      </span>

      {summary && summary.totalActiveSecs > 0 && (
        <div className="w-full flex flex-col gap-1.5 mt-2">
          <ScoreBar label="Focus" value={summary.productiveSecs / summary.totalActiveSecs} />
          <ScoreBar label="Quality" value={summary.avgSessionQuality ?? 0} />
          <ScoreBar
            label="Low distraction"
            value={1 - summary.distractingSecs / Math.max(summary.totalActiveSecs, 1)}
          />
          <ScoreBar
            label="Continuity"
            value={summary.contextSwitches > 0 ? Math.max(0, 1 - summary.contextSwitches / 100) : 1}
          />
        </div>
      )}
    </div>
  );
}
