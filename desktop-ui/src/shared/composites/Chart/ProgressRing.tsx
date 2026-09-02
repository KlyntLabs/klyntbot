import { cn } from "@shared/lib/utils";
import { useEffect, useState } from "react";

export interface ProgressRingProps {
  value: number;
  size?: number;
  strokeWidth?: number;
  color?: string;
  label?: string;
  animated?: boolean;
  className?: string;
}

export function ProgressRing({
  value,
  size = 110,
  strokeWidth = 7,
  color = "var(--ds-accent)",
  label,
  animated = true,
  className,
}: ProgressRingProps) {
  const [mounted, setMounted] = useState(!animated);

  useEffect(() => {
    if (!animated) return;
    const id = requestAnimationFrame(() => setMounted(true));
    return () => cancelAnimationFrame(id);
  }, [animated]);

  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const progress = Math.min(Math.max(value / 100, 0), 1);
  const offset = circumference * (1 - progress);
  const center = size / 2;

  return (
    <div className={cn("flex flex-col items-center gap-2", className)}>
      <div className="relative" style={{ width: size, height: size }}>
        <div
          className="absolute inset-2 rounded-full transition-opacity duration-700"
          style={{
            background: `radial-gradient(circle, ${color}15 0%, transparent 70%)`,
            opacity: value > 0 ? 1 : 0,
          }}
        />
        <svg width={size} height={size} className="-rotate-90" aria-hidden="true">
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke="var(--ds-glass-bg-subtle)"
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
            strokeDashoffset={mounted ? offset : circumference}
            className="transition-[stroke-dashoffset] duration-1000"
            style={{
              filter: `drop-shadow(0 0 4px ${color}66)`,
            }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-[26px] font-light tabular-nums leading-none" style={{ color }}>
            {Math.round(value)}
          </span>
          <span className="text-[9px] font-light text-fg-dim mt-0.5">/100</span>
        </div>
      </div>
      {label && (
        <span className="text-[10px] font-medium tracking-wide uppercase" style={{ color }}>
          {label}
        </span>
      )}
    </div>
  );
}
