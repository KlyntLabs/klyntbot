import { formatDate, formatRelativeTime, formatTime } from "@shared/lib/dates";
import { Bot, Monitor, User } from "lucide-react";
import type { ActivityEntry as ActivityEntryType, ActorType } from "../../lib/mappers";

const DAY_MS = 24 * 60 * 60 * 1000;

function formatActivityTime(iso: string): string {
  const age = Date.now() - new Date(iso).getTime();
  if (age < DAY_MS) return formatRelativeTime(iso);
  const d = new Date(iso);
  return `${formatDate(d.toISOString().slice(0, 10))} ${formatTime(iso)}`;
}

interface IssueActivityTabProps {
  activity: ActivityEntryType[];
}

export function IssueActivityTab({ activity }: IssueActivityTabProps) {
  return (
    <div className="space-y-0">
      {activity.map((entry) => (
        <ActivityEntry key={entry.id} entry={entry} />
      ))}
    </div>
  );
}

function ActivityEntry({ entry }: { entry: ActivityEntryType }) {
  return (
    <div className="flex gap-3 py-3 border-b border-[hsl(var(--border))]/50 last:border-b-0">
      <ActorAvatar type={entry.actorType} />
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-medium text-[hsl(var(--foreground))]">
            {entry.actorName}
          </span>
          <span className="text-sm text-[hsl(var(--muted-foreground))]">{entry.action}</span>
          <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))] shrink-0">
            {formatActivityTime(entry.createdAt)}
          </span>
        </div>
        {entry.detail && (
          <p className="text-sm text-[hsl(var(--muted-foreground))] mt-0.5">{entry.detail}</p>
        )}
      </div>
    </div>
  );
}

function ActorAvatar({ type }: { type: ActorType }) {
  if (type === "agent") {
    return (
      <div className="size-7 rounded-full shrink-0 flex items-center justify-center bg-gradient-to-br from-purple-500 to-indigo-600">
        <Bot className="size-3.5 text-white" />
      </div>
    );
  }
  if (type === "system") {
    return (
      <div className="size-7 rounded-full shrink-0 flex items-center justify-center bg-[hsl(var(--muted))]">
        <Monitor className="size-3.5 text-[hsl(var(--muted-foreground))]" />
      </div>
    );
  }
  return (
    <div className="size-7 rounded-full shrink-0 flex items-center justify-center bg-[hsl(var(--accent))]">
      <User className="size-3.5 text-[hsl(var(--foreground))]" />
    </div>
  );
}
