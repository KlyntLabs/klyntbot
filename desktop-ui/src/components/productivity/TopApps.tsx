import { useMemo } from "react";
import { formatHumanDuration } from "../../lib/dates";
import type { AppUsage } from "../../lib/types";
import { getCategoryColor } from "./shared";

interface TopAppsProps {
  apps: AppUsage[];
}

export function TopApps({ apps }: TopAppsProps) {
  const { maxDuration, totalDuration } = useMemo(
    () =>
      apps.reduce(
        (acc, a) => ({ max: Math.max(acc.max, a.durationSecs), total: acc.total + a.durationSecs }),
        { max: 1, total: 0 },
      ),
    [apps],
  );

  if (apps.length === 0) {
    return (
      <div className="glass-card p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Top Apps</h2>
        <p className="text-[12px] font-light text-dim">No app data yet</p>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Top Apps</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">{apps.length} tracked</span>
      </div>
      <div className="flex flex-col gap-1.5">
        {apps.slice(0, 10).map((app, i) => {
          const pct = totalDuration > 0 ? Math.round((app.durationSecs / totalDuration) * 100) : 0;
          const widthPct = (app.durationSecs / maxDuration) * 100;
          const color = app.category ? getCategoryColor(app.category) : "var(--brand)";
          const isTop3 = i < 3;

          return (
            <div
              key={app.appName}
              className="group flex items-center gap-2.5 py-1 rounded-md px-1 -mx-1 hover:bg-white/[0.05] transition-colors"
            >
              {/* Rank + dot */}
              <div className="flex items-center gap-1.5 w-10 flex-shrink-0">
                <span
                  className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                  style={{ backgroundColor: color, opacity: isTop3 ? 1 : 0.5 }}
                />
                <span
                  className={`text-[10px] tabular-nums ${isTop3 ? "font-medium text-muted" : "font-light text-dim"}`}
                >
                  {pct}%
                </span>
              </div>

              {/* App name */}
              <span
                className={`text-[11px] w-28 truncate flex-shrink-0 ${isTop3 ? "font-normal text-primary" : "font-light text-secondary"}`}
              >
                {app.appName}
              </span>

              {/* Bar */}
              <div className="flex-1 h-2 rounded-full bg-white/[0.08] overflow-hidden relative">
                <div
                  className="h-full rounded-full transition-[width] duration-500"
                  style={{
                    width: `${widthPct}%`,
                    background: `linear-gradient(90deg, ${color}, ${color}dd)`,
                    boxShadow: isTop3 ? `0 0 8px ${color}33` : undefined,
                  }}
                />
              </div>

              {/* Duration */}
              <span
                className={`text-[11px] tabular-nums w-14 text-right flex-shrink-0 ${isTop3 ? "font-normal text-muted" : "font-light text-dim"}`}
              >
                {formatHumanDuration(app.durationSecs)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
