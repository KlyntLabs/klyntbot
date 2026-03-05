export function Donut({
  segments,
  label,
  value,
  size = 140,
}: {
  segments: { name: string; value: number; color: string }[];
  label: string;
  value: string;
  size?: number;
}) {
  const total = segments.reduce((s, seg) => s + seg.value, 0);
  if (total === 0) return null;
  const r = size / 2 - 12;
  const cx = size / 2;
  const cy = size / 2;
  const sw = 16;
  const circ = 2 * Math.PI * r;
  let off = 0;

  return (
    <div className="flex flex-col items-center">
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        {segments.map((seg, i) => {
          const frac = seg.value / total;
          const dash = frac * circ;
          const rot = (off / total) * 360 - 90;
          off += seg.value;
          return (
            <circle
              key={i}
              cx={cx}
              cy={cy}
              r={r}
              fill="none"
              stroke={seg.color}
              strokeWidth={sw}
              strokeDasharray={`${dash} ${circ - dash}`}
              transform={`rotate(${rot} ${cx} ${cy})`}
            />
          );
        })}
        <text
          x={cx}
          y={cy - 5}
          textAnchor="middle"
          className="fill-dim text-[9px]"
          style={{ fontWeight: 300 }}
        >
          {label}
        </text>
        <text
          x={cx}
          y={cy + 10}
          textAnchor="middle"
          className="fill-primary text-[13px]"
          style={{ fontWeight: 300 }}
        >
          {value}
        </text>
      </svg>
      <div className="flex flex-wrap gap-x-3 gap-y-0.5 justify-center mt-2">
        {segments.map((seg, i) => (
          <div key={i} className="flex items-center gap-1">
            <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: seg.color }} />
            <span className="text-[9px] text-dim font-light">{seg.name}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
