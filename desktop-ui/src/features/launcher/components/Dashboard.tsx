import { useLauncherStore } from "../stores/launcherStore";
import type { DashboardData } from "../types";

interface DashboardProps {
  onOpenTask?: (taskId: string) => void;
}

export function Dashboard({ onOpenTask }: DashboardProps) {
  const dashboard = useLauncherStore((s) => s.dashboard);

  if (!dashboard) {
    return (
      <div className="p-6 text-center text-muted-foreground text-sm animate-pulse">Loading...</div>
    );
  }

  const hasContent =
    dashboard.calendar.length > 0 ||
    dashboard.tasks.length > 0 ||
    dashboard.productivity.totalMinutes > 0;

  if (!hasContent) {
    return (
      <div className="px-4 py-6 text-center">
        <p className="text-sm text-muted-foreground">No activity yet today</p>
        <p className="text-xs text-dim mt-1">Start a focus session or search for anything</p>
      </div>
    );
  }

  return (
    <div className="px-3 pt-2 pb-3 space-y-2 max-h-[460px] overflow-y-auto">
      {dashboard.calendar.length > 0 && <CalendarWidget events={dashboard.calendar} />}
      {dashboard.tasks.length > 0 && (
        <TasksWidget tasks={dashboard.tasks} onOpenTask={onOpenTask} />
      )}
      {dashboard.productivity.totalMinutes > 0 && (
        <ProductivityWidget productivity={dashboard.productivity} />
      )}
    </div>
  );
}

/* ── Calendar ─────────────────────────────────────────────────── */

function CalendarWidget({ events }: { events: DashboardData["calendar"] }) {
  return (
    <div className="rounded-lg border border-border-subtle bg-card/50 p-3">
      <div className="flex items-center gap-1.5 mb-2">
        <svg
          className="size-3.5 text-info/70"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <title>Calendar</title>
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
          />
        </svg>
        <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
          Upcoming
        </span>
      </div>
      <div className="space-y-1">
        {events.map((event) => {
          const isNow = event.minutesUntil <= 0;
          const isSoon = event.minutesUntil > 0 && event.minutesUntil <= 15;
          return (
            <div key={event.eventId} className="flex items-center gap-2 py-0.5">
              <div
                className={`w-[3px] h-4 rounded-full shrink-0 ${
                  isNow ? "bg-destructive" : isSoon ? "bg-warning" : "bg-info/40"
                }`}
              />
              <span className="text-[13px] text-foreground/90 truncate flex-1">{event.title}</span>
              <span
                className={`text-[11px] tabular-nums shrink-0 ${
                  isNow
                    ? "text-destructive font-medium"
                    : isSoon
                      ? "text-warning"
                      : "text-muted-foreground"
                }`}
              >
                {isNow ? "Now" : `${event.minutesUntil}m`}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/* ── Tasks ─────────────────────────────────────────────────────── */

const STATUS_COLORS: Record<string, string> = {
  doing: "bg-brand",
  todo: "bg-muted/50",
  blocked: "bg-destructive",
};

function TasksWidget({
  tasks,
  onOpenTask,
}: {
  tasks: DashboardData["tasks"];
  onOpenTask?: (id: string) => void;
}) {
  return (
    <div className="rounded-lg border border-border-subtle bg-card/50 p-3">
      <div className="flex items-center gap-1.5 mb-2">
        <svg
          className="size-3.5 text-success/70"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <title>Tasks</title>
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
          Tasks
        </span>
        <span className="text-[11px] text-dim ml-auto">{tasks.length}</span>
      </div>
      <div className="space-y-0.5">
        {tasks.map((task) => (
          <button
            key={task.id}
            type="button"
            onClick={() => onOpenTask?.(task.id)}
            className="flex items-center gap-2.5 py-1 group w-full text-left rounded-md px-1 -mx-1 hover:bg-accent active:bg-accent transition-colors cursor-pointer"
          >
            <div
              className={`w-[7px] h-[7px] rounded-full shrink-0 ring-1 ring-inset ring-white/10 ${
                STATUS_COLORS[task.status] || "bg-muted/40"
              }`}
            />
            <span className="text-[13px] text-foreground/85 truncate flex-1 group-hover:text-foreground transition-colors">
              {task.title}
            </span>
            {task.status === "doing" && (
              <span className="text-2xs text-brand/70 font-medium uppercase tracking-wide shrink-0">
                Active
              </span>
            )}
            <svg
              className="size-3.5 text-muted-foreground/0 group-hover:text-muted-foreground/60 transition-colors shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <title>Open</title>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
            </svg>
          </button>
        ))}
      </div>
    </div>
  );
}

/* ── Productivity ──────────────────────────────────────────────── */

function ScoreRing({ score, size = 44 }: { score: number; size?: number }) {
  const stroke = 3;
  const r = (size - stroke) / 2;
  const circumference = 2 * Math.PI * r;
  const offset = circumference - (Math.min(score, 100) / 100) * circumference;

  // Color based on score
  const color =
    score >= 70 ? "var(--success)" : score >= 40 ? "var(--brand)" : "var(--destructive)";

  return (
    <svg
      width={size}
      height={size}
      className="shrink-0 -rotate-90"
      role="img"
      aria-label={`Productivity score: ${score}`}
    >
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="rgba(255,255,255,0.06)"
        strokeWidth={stroke}
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke={color}
        strokeWidth={stroke}
        strokeLinecap="round"
        strokeDasharray={circumference}
        strokeDashoffset={offset}
        style={{ transition: "stroke-dashoffset 0.8s ease-out" }}
      />
      <text
        x={size / 2}
        y={size / 2}
        textAnchor="middle"
        dominantBaseline="central"
        fill="var(--text-primary)"
        fontSize="13"
        fontWeight="600"
        fontFamily="inherit"
        style={{ transform: "rotate(90deg)", transformOrigin: "center" }}
      >
        {score}
      </text>
    </svg>
  );
}

function ProductivityWidget({ productivity }: { productivity: DashboardData["productivity"] }) {
  const hours = Math.floor(productivity.totalMinutes / 60);
  const mins = productivity.totalMinutes % 60;
  const timeStr = hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;

  return (
    <div className="rounded-lg border border-border-subtle bg-card/50 p-3">
      <div className="flex items-center gap-3">
        <ScoreRing score={productivity.score} />
        <div className="flex-1 min-w-0">
          <div className="flex items-baseline gap-1.5">
            <span className="text-[13px] font-medium text-foreground">{timeStr}</span>
            <span className="text-[11px] text-muted-foreground">today</span>
          </div>
          <div className="mt-1.5">
            <div className="flex items-center justify-between mb-0.5">
              <span className="text-[11px] text-muted-foreground">{productivity.topCategory}</span>
              <span className="text-[11px] tabular-nums text-dim">
                {Math.round(productivity.topCategoryPct)}%
              </span>
            </div>
            <div className="h-[3px] bg-accent rounded-full overflow-hidden">
              <div
                className="h-full rounded-full transition-all duration-700 ease-out"
                style={{
                  width: `${productivity.topCategoryPct}%`,
                  background:
                    "linear-gradient(90deg, rgba(255,255,255,0.15) 0%, rgba(255,255,255,0.3) 100%)",
                }}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
