import { useMemo } from "react";
import { formatHumanDuration } from "../../lib/dates";
import type { AppUsage } from "../../lib/types";

interface TopAppsProps {
  apps: AppUsage[];
}

export function TopApps({ apps }: TopAppsProps) {
  const maxDuration = useMemo(
    () => apps.reduce((max, a) => Math.max(max, a.durationSecs), 1),
    [apps],
  );
  const totalDuration = useMemo(() => apps.reduce((sum, a) => sum + a.durationSecs, 0), [apps]);

  if (apps.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Top Apps</h2>
        <p className="text-[12px] font-light text-dim">No app data yet</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Top Apps</h2>
      <div className="flex flex-col gap-2">
        {apps.slice(0, 10).map((app) => {
          const pct = totalDuration > 0 ? Math.round((app.durationSecs / totalDuration) * 100) : 0;
          return (
            <div key={app.appName} className="flex items-center gap-3">
              <span className="text-[11px] font-light text-muted w-8 text-right tabular-nums">
                {pct}%
              </span>
              <span className="text-[11px] font-light text-primary flex-1 truncate">
                {app.appName}
              </span>
              <div className="w-20 h-1.5 rounded-full bg-surface-raised overflow-hidden flex-shrink-0">
                <div
                  className="h-full rounded-full bg-brand"
                  style={{ width: `${(app.durationSecs / maxDuration) * 100}%` }}
                />
              </div>
              <span className="text-[11px] font-light text-muted tabular-nums w-16 text-right">
                {formatHumanDuration(app.durationSecs)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
