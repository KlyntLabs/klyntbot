import { formatRelativeTime } from "@shared/lib/dates";
import { useMemo } from "react";
import { useProjectContext } from "../../contexts/ProjectContext";

interface TimelineItem {
  id: string;
  type: "task" | "objective" | "note";
  label: string;
  timestamp: string;
}

const DOT_COLORS: Record<string, string> = {
  task: "#10b981", // green
  objective: "#eab308", // yellow
  note: "#a855f7", // purple
};

function groupLabel(ts: string): string {
  const date = new Date(ts);
  const now = new Date();
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const weekAgo = new Date(todayStart);
  weekAgo.setDate(weekAgo.getDate() - 7);

  if (date >= todayStart) return "Today";
  if (date >= weekAgo) return "This Week";
  return "Earlier";
}

const MAX_ITEMS = 10;

export function ActivityTimeline() {
  const { tasks } = useProjectContext();

  const grouped = useMemo(() => {
    const all: TimelineItem[] = [];

    for (const t of tasks) {
      if (t.updatedAt) {
        all.push({
          id: t.id,
          type: "task",
          label: `${t.completed ? "Completed" : "Updated"}: ${t.title}`,
          timestamp: t.updatedAt,
        });
      }
    }

    // Objectives excluded from timeline — they don't have updatedAt
    // and using fabricated timestamps (new Date()) causes incorrect ordering.

    all.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
    const items = all.slice(0, MAX_ITEMS);

    const groups = new Map<string, TimelineItem[]>();
    for (const item of items) {
      const group = groupLabel(item.timestamp);
      const existing = groups.get(group);
      if (existing) existing.push(item);
      else groups.set(group, [item]);
    }
    return groups;
  }, [tasks]);

  if (grouped.size === 0) {
    return (
      <div className="glass-card rounded-xl p-5">
        <p className="text-2xs text-muted-foreground uppercase tracking-wider mb-3">
          Recent Activity
        </p>
        <p className="text-[11px] text-muted-foreground">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="glass-card rounded-xl p-5">
      <p className="text-2xs text-muted-foreground uppercase tracking-wider mb-4">
        Recent Activity
      </p>

      <div className="flex flex-col gap-4">
        {Array.from(grouped.entries()).map(([group, groupItems]) => (
          <div key={group}>
            <p className="text-2xs text-muted-foreground font-medium mb-2">{group}</p>
            <div className="flex flex-col gap-2">
              {groupItems.map((item) => (
                <div key={item.id} className="flex items-start gap-3">
                  <div className="flex flex-col items-center mt-1.5">
                    <div
                      className="size-2 rounded-full flex-shrink-0"
                      style={{ backgroundColor: DOT_COLORS[item.type] }}
                    />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-xs text-foreground truncate">{item.label}</p>
                    <p className="text-2xs text-muted-foreground">
                      {formatRelativeTime(item.timestamp)}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
