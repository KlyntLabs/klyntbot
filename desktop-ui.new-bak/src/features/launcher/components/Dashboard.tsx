import { useDashboardData } from "../hooks/useDashboardData";
import type { DashboardData } from "../types";

interface DashboardProps {
  onOpenTask?: (taskId: string) => void;
}

function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleTimeString([], {
      hour: "numeric",
      minute: "2-digit",
    });
  } catch {
    return "";
  }
}

export function Dashboard({ onOpenTask }: DashboardProps) {
  const { data: dashboard } = useDashboardData();

  if (!dashboard) return null;

  const hasContent = dashboard.calendar.length > 0 || dashboard.tasks.length > 0;

  if (!hasContent) return null;

  return (
    <div className="lc-dash">
      {dashboard.calendar.length > 0 && <CalendarWidget events={dashboard.calendar} />}
      {dashboard.tasks.length > 0 && (
        <TasksWidget tasks={dashboard.tasks} onOpenTask={onOpenTask} />
      )}
    </div>
  );
}

function CalendarWidget({ events }: { events: DashboardData["calendar"] }) {
  return (
    <div className="lc-card">
      <div className="lc-card-header">
        <span className="lc-card-title">Upcoming</span>
      </div>
      <div className="lc-card-body">
        {events.map((event) => {
          const isNow = event.minutesUntil <= 0;
          const isSoon = event.minutesUntil > 0 && event.minutesUntil <= 15;
          const dotClass = isNow ? "is-now" : isSoon ? "is-soon" : "";
          return (
            <div key={event.eventId} className="lc-cal-row">
              <div className={`lc-cal-dot ${dotClass}`} />
              <span className="lc-cal-title">{event.title}</span>
              <span className={`lc-cal-time ${dotClass}`}>
                {isNow ? "Now" : `${event.minutesUntil}m`}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function TasksWidget({
  tasks,
  onOpenTask,
}: {
  tasks: DashboardData["tasks"];
  onOpenTask?: (id: string) => void;
}) {
  return (
    <div className="lc-card">
      <div className="lc-card-header">
        <span className="lc-card-title">Tasks</span>
        <span className="lc-card-count">{tasks.length}</span>
      </div>
      <div className="lc-card-body">
        {tasks.map((task) => (
          <button
            key={task.id}
            type="button"
            onClick={() => onOpenTask?.(task.id)}
            className="lc-task-row"
          >
            <div className={`lc-task-dot status-${task.status}`} />
            <span className="lc-task-title">{task.title}</span>
            {task.dueDate && <span className="lc-task-time">{formatTime(task.dueDate)}</span>}
            {task.status === "doing" && <span className="lc-task-active">Active</span>}
          </button>
        ))}
      </div>
    </div>
  );
}
