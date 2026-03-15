import { useEffect, useState } from "react";
import type { HealthScore } from "../lib/healthScore";
import { scoreColor } from "../lib/healthScore";

export function HealthScoreRing({ health }: { health: HealthScore }) {
  const [animated, setAnimated] = useState(false);
  useEffect(() => {
    const id = requestAnimationFrame(() => setAnimated(true));
    return () => cancelAnimationFrame(id);
  }, []);

  const color = scoreColor(health.score);
  const r = 50;
  const circ = 2 * Math.PI * r;
  const filled = (health.score / 100) * circ;

  return (
    <div className="flex items-center gap-6">
      <div className="flex-shrink-0">
        <svg width={120} height={120} viewBox="0 0 120 120" aria-hidden="true">
          <circle cx={60} cy={60} r={r} fill="none" stroke="rgba(255,255,255,0.06)" strokeWidth={8} />
          <circle
            cx={60} cy={60} r={r} fill="none" stroke={color} strokeWidth={8}
            strokeDasharray={`${filled} ${circ - filled}`}
            strokeDashoffset={animated ? 0 : circ}
            strokeLinecap="round"
            transform="rotate(-90 60 60)"
            style={{
              transition: "stroke-dashoffset 1s ease-out",
              filter: `drop-shadow(0 0 10px ${color}50)`,
            }}
          />
          <text x={60} y={54} textAnchor="middle" className="fill-primary text-[30px]" style={{ fontWeight: 200, fontVariantNumeric: "tabular-nums" }}>
            {health.score}
          </text>
          <text x={60} y={70} textAnchor="middle" className="fill-muted text-[9px]" style={{ letterSpacing: "0.05em" }}>
            HEALTH
          </text>
        </svg>
      </div>
      <div className="flex-1">
        <p className="text-[11px] font-normal mb-3" style={{ color: health.statusColor }}>
          {health.status}
        </p>
        {health.factors.map((f) => (
          <div key={f.name} className="mb-2.5 last:mb-0">
            <div className="flex justify-between text-[11px] mb-1">
              <span className="text-secondary">{f.name}</span>
              <span style={{ color: f.color }}>{f.value}%</span>
            </div>
            <div className="h-1 bg-white/[0.06] rounded-full">
              <div className="h-full rounded-full" style={{ width: `${f.value}%`, background: f.color, transition: "width 0.8s ease" }} />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
