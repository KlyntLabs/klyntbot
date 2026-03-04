import { useMemo } from 'react';
import { formatHumanDuration } from '../../lib/dates';
import type { AppUsage } from '../../lib/types';

interface TopAppsProps {
  apps: AppUsage[];
}

export function TopApps({ apps }: TopAppsProps) {
  const maxDuration = useMemo(() => apps.reduce((max, a) => Math.max(max, a.durationSecs), 1), [apps]);

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
        {apps.slice(0, 8).map(app => (
          <div key={app.appName} className="flex flex-col gap-1">
            <div className="flex items-center justify-between text-[11px] font-light">
              <span className="text-primary truncate">{app.appName}</span>
              <span className="text-muted tabular-nums">{formatHumanDuration(app.durationSecs)}</span>
            </div>
            <div className="h-1 rounded-full bg-surface-raised overflow-hidden">
              <div
                className="h-full rounded-full bg-brand"
                style={{ width: `${(app.durationSecs / maxDuration) * 100}%` }}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
