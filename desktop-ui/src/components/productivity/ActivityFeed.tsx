import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import { formatTime } from "../../lib/dates";
import type { ActivityTimeline } from "../../lib/types";
import { getCategoryColor } from "./shared";

function dotColor(categoryId: string | null, isIdle: boolean): string {
  if (isIdle) return "var(--surface-highest)";
  if (categoryId) return getCategoryColor(categoryId);
  return "var(--brand)";
}

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
      <div className="glass-card p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Activity</h2>
        <p className="text-[12px] font-light text-dim">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Activity</h2>
        <div className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
          <span className="text-[10px] font-light text-dim">Live</span>
        </div>
      </div>
      <div className="flex flex-col gap-0 max-h-64 overflow-y-auto">
        {events.map((e, i) => {
          const color = dotColor(e.categoryId, e.isIdle);
          const isFirst = i === 0;
          return (
            <div
              key={`${e.startedAt}-${e.appName}`}
              className={`flex items-center gap-2.5 py-1.5 ${isFirst ? "" : "border-t border-white/[0.04]/50"}`}
            >
              {/* Timeline dot */}
              <span
                className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                style={{ backgroundColor: color }}
              />

              {/* Time */}
              <span className="text-[10px] text-dim tabular-nums w-14 flex-shrink-0 font-light">
                {formatTime(e.startedAt)}
              </span>

              {/* App/Site info */}
              <div className="flex-1 min-w-0 flex items-center gap-1.5">
                <span
                  className={`text-[11px] truncate ${e.isIdle ? "text-dim italic" : isFirst ? "font-normal text-primary" : "font-light text-secondary"}`}
                >
                  {e.isIdle ? "Idle" : (e.siteName ?? e.appName)}
                </span>
                {e.siteName && !e.isIdle && (
                  <span className="text-[10px] font-light text-dim truncate">{e.appName}</span>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
