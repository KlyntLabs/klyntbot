interface ProgressRingProps {
  progress: number;
  size: "sm" | "md" | "lg";
  color?: string;
  gradient?: boolean;
  className?: string;
}

const SIZES = { sm: 28, md: 48, lg: 80 } as const;
const STROKE = { sm: 2, md: 3, lg: 3.5 } as const;
const RADIUS = 15.5;
// pathLength="100" on the SVG circle means strokeDasharray works as direct percentages

export function ProgressRing({ progress, size, color, gradient, className }: ProgressRingProps) {
  const px = SIZES[size];
  const sw = STROKE[size];
  const clamped = Math.max(0, Math.min(100, progress));
  const dasharray = `${clamped} ${100 - clamped}`;
  const gradientId = `pr-grad-${size}`;

  const strokeColor = gradient ? `url(#${gradientId})` : (color ?? "var(--brand)");

  return (
    <svg width={px} height={px} viewBox="0 0 36 36" className={className}>
      {gradient && (
        <defs>
          <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#10b981" />
            <stop offset="100%" stopColor="#6366f1" />
          </linearGradient>
        </defs>
      )}
      <circle
        cx="18"
        cy="18"
        r={RADIUS}
        fill="none"
        stroke="rgba(255,255,255,0.06)"
        strokeWidth={sw}
      />
      <circle
        cx="18"
        cy="18"
        r={RADIUS}
        fill="none"
        stroke={strokeColor}
        strokeWidth={sw}
        strokeDasharray={dasharray}
        strokeLinecap="round"
        transform="rotate(-90 18 18)"
        pathLength="100"
      />
      {size !== "sm" && (
        <text
          x="18"
          y={size === "lg" ? 19 : 20}
          textAnchor="middle"
          fill="currentColor"
          fontSize={size === "lg" ? 8 : 9}
          fontWeight="700"
        >
          {Math.round(clamped)}%
        </text>
      )}
    </svg>
  );
}
