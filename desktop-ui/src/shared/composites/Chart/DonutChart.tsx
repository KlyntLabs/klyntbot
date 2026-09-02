import { useEffect, useState } from "react";

export interface DonutSegment {
  name: string;
  value: number;
  color: string;
}

export interface DonutChartProps {
  segments: DonutSegment[];
  label?: string;
  value?: string;
  size?: number;
  strokeWidth?: number;
  children?: React.ReactNode;
}

export function DonutChart({
  segments,
  label,
  value,
  size = 140,
  strokeWidth = 16,
  children,
}: DonutChartProps) {
  const [animated, setAnimated] = useState(false);

  useEffect(() => {
    const id = requestAnimationFrame(() => setAnimated(true));
    return () => cancelAnimationFrame(id);
  }, []);

  const total = segments.reduce((s, seg) => s + seg.value, 0);
  if (total === 0) return null;

  const r = size / 2 - 12;
  const cx = size / 2;
  const cy = size / 2;
  const sw = strokeWidth;
  const circ = 2 * Math.PI * r;
  let off = 0;

  return (
    <div className="flex flex-col items-center">
      <div className="relative" style={{ width: size, height: size }}>
        <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden="true">
          {segments.map((seg, i) => {
            const frac = seg.value / total;
            const dash = frac * circ;
            const rot = (off / total) * 360 - 90;
            off += seg.value;
            return (
              <circle
                key={seg.name}
                cx={cx}
                cy={cy}
                r={r}
                fill="none"
                stroke={seg.color}
                strokeWidth={sw}
                strokeDasharray={`${dash} ${circ - dash}`}
                strokeDashoffset={animated ? 0 : dash}
                transform={`rotate(${rot} ${cx} ${cy})`}
                style={{
                  transition: `stroke-dashoffset 700ms ease-out ${i * 80}ms`,
                  filter: `drop-shadow(0 0 4px ${seg.color}40)`,
                }}
              />
            );
          })}
          {label && (
            <text
              x={cx}
              y={cy - 5}
              textAnchor="middle"
              className="fill-fg-dim text-[9px]"
              style={{ fontWeight: 300 }}
            >
              {label}
            </text>
          )}
          {value && (
            <text
              x={cx}
              y={cy + 10}
              textAnchor="middle"
              className="fill-fg text-ui"
              style={{ fontWeight: 300 }}
            >
              {value}
            </text>
          )}
        </svg>
        {children && (
          <div className="absolute inset-0 flex items-center justify-center">{children}</div>
        )}
      </div>
      <div className="flex flex-wrap gap-x-3 gap-y-0.5 justify-center mt-2">
        {segments.map((seg) => (
          <div key={seg.name} className="flex items-center gap-1">
            <div className="size-1.5 rounded-full" style={{ backgroundColor: seg.color }} />
            <span className="text-[9px] text-fg-dim font-light">{seg.name}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
