import { useLauncherStore } from "../stores/launcherStore";
import type { DashboardData } from "../types";

export function Dashboard() {
  const dashboard = useLauncherStore((s) => s.dashboard);

  if (!dashboard) {
    return <div className="p-4 text-center text-muted text-sm">Loading dashboard...</div>;
  }

  return (
    <div className="p-3 space-y-3 max-h-[500px] overflow-y-auto">
      {dashboard.focus && <FocusWidget focus={dashboard.focus} />}
      {dashboard.calendar.length > 0 && <CalendarWidget events={dashboard.calendar} />}
      {dashboard.tasks.length > 0 && <TasksWidget tasks={dashboard.tasks} />}
      <ProductivityWidget productivity={dashboard.productivity} />
    </div>
  );
}

function FocusWidget({ focus }: { focus: NonNullable<DashboardData["focus"]> }) {
  const mins = Math.floor(focus.elapsedSecs / 60);
  const secs = focus.elapsedSecs % 60;
  const progress = focus.targetSecs
    ? Math.min(100, (focus.elapsedSecs / focus.targetSecs) * 100)
    : null;

  return (
    <div className="rounded-xl bg-surface-raised p-3">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs font-medium text-muted">Focus Session</span>
        <span className="text-xs tabular-nums text-foreground">
          {mins}:{String(secs).padStart(2, "0")}
        </span>
      </div>
      {focus.taskName && <p className="text-sm text-foreground truncate">{focus.taskName}</p>}
      {progress !== null && (
        <div className="mt-2 h-1 bg-surface-base rounded-full overflow-hidden">
          <div
            className="h-full bg-accent rounded-full transition-all"
            style={{ width: `${progress}%` }}
          />
        </div>
      )}
    </div>
  );
}

function CalendarWidget({ events }: { events: DashboardData["calendar"] }) {
  return (
    <div className="rounded-xl bg-surface-raised p-3">
      <span className="text-xs font-medium text-muted">Upcoming</span>
      <div className="mt-2 space-y-2">
        {events.map((event) => (
          <div key={event.eventId} className="flex items-center justify-between">
            <span className="text-sm text-foreground truncate">{event.title}</span>
            <span className="text-xs text-muted shrink-0 ml-2">
              {event.minutesUntil <= 0 ? "Now" : `in ${event.minutesUntil}m`}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function TasksWidget({ tasks }: { tasks: DashboardData["tasks"] }) {
  return (
    <div className="rounded-xl bg-surface-raised p-3">
      <span className="text-xs font-medium text-muted">Top Tasks</span>
      <div className="mt-2 space-y-1.5">
        {tasks.map((task) => (
          <div key={task.id} className="flex items-center gap-2">
            <div className="w-1.5 h-1.5 rounded-full bg-accent shrink-0" />
            <span className="text-sm text-foreground truncate">{task.title}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ProductivityWidget({ productivity }: { productivity: DashboardData["productivity"] }) {
  const hours = Math.floor(productivity.totalMinutes / 60);
  const mins = productivity.totalMinutes % 60;

  return (
    <div className="rounded-xl bg-surface-raised p-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-muted">Today</span>
        <span className="text-xs text-foreground">
          {hours > 0 ? `${hours}h ${mins}m` : `${mins}m`}
        </span>
      </div>
      <div className="mt-2 flex items-center gap-3">
        <div className="flex-1">
          <div className="h-1 bg-surface-base rounded-full overflow-hidden">
            <div
              className="h-full bg-accent rounded-full"
              style={{ width: `${productivity.topCategoryPct}%` }}
            />
          </div>
          <span className="text-xs text-muted mt-1">{productivity.topCategory}</span>
        </div>
        <span className="text-lg font-semibold text-foreground tabular-nums">
          {productivity.score}
        </span>
      </div>
    </div>
  );
}
