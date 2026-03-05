import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import { formatTime } from "../../lib/dates";
import type { ActivityTimeline } from "../../lib/types";

export function ActivityFeed() {
  const { data: events, refetch } = useQuery<ActivityTimeline[]>(
    "productivity_activity_feed",
    { limit: 30 },
    [],
  );

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "productivity") refetch();
  });

  if (events.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Activity</h2>
        <p className="text-[12px] font-light text-dim">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Activity</h2>
        <span className="text-[10px] font-light text-dim">Tracking: Ok</span>
      </div>
      <div className="flex flex-col gap-0.5 max-h-64 overflow-y-auto">
        {events.map((e) => (
          <div
            key={`${e.startedAt}-${e.appName}`}
            className="flex items-center gap-2 py-1 text-[11px] font-light"
          >
            <span className="text-dim tabular-nums w-14 flex-shrink-0">
              {formatTime(e.startedAt)}
            </span>
            <span className="text-primary truncate">{e.appName}</span>
            {e.windowTitle && <span className="text-dim truncate flex-1">{e.windowTitle}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}
