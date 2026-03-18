import type { PulseRow } from "../lib/monthlyPulse";
import { DIRECTION_ICONS } from "../lib/monthlyPulse";

export function MonthlyPulse({ rows }: { rows: PulseRow[] }) {
  return (
    <div className="flex flex-col justify-center h-full">
      <p className="text-[10px] text-muted-foreground uppercase tracking-widest mb-4">
        Monthly Pulse
      </p>
      {rows.map((row) => (
        <div key={row.label} className="flex items-center gap-3 mb-3.5 last:mb-0">
          <div
            className="w-9 h-9 rounded-xl flex items-center justify-center text-[16px] font-light flex-shrink-0"
            style={{ background: `${row.color}18`, color: row.color }}
          >
            {DIRECTION_ICONS[row.direction]}
          </div>
          <div className="flex-1">
            <p className="text-[11px] text-muted-foreground">{row.label}</p>
            <p className="text-[10px] text-dim mt-0.5">{row.hint}</p>
            <div className="h-1 bg-accent rounded-full mt-1.5">
              <div
                className="h-full rounded-full"
                style={{
                  width: `${row.barWidth}%`,
                  background: row.color,
                  transition: "width 0.8s ease",
                }}
              />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
