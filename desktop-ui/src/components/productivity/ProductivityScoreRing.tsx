import { scoreColor } from "./shared";

interface ProductivityScoreRingProps {
  score: number;
  size?: number;
}

function scoreLabel(score: number): string {
  if (score >= 80) return "Excellent";
  if (score >= 60) return "Good";
  if (score >= 40) return "Fair";
  if (score > 0) return "Low";
  return "—";
}

export function ProductivityScoreRing({ score, size = 110 }: ProductivityScoreRingProps) {
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

        <svg width={size} height={size} className="-rotate-90">
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
    </div>
  );
}
