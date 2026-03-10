import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration } from "@shared/lib/dates";
import type { Project, Task } from "@shared/types";
import {
  CalendarDays,
  CheckCircle2,
  Circle,
  ListTodo,
  Target,
} from "lucide-react";

interface ProjectSummaryPanelProps {
  project: Project;
}

export function ProjectSummaryPanel({ project }: ProjectSummaryPanelProps) {
  const { data: tasks } = useQuery<Task[]>(
    "task_list",
    { projectId: project.id },
    [],
  );

  const total = tasks.length;
  const completed = tasks.filter((t) => t.status === "done").length;
  const inProgress = tasks.filter((t) => t.status === "in_progress").length;
  const pending = total - completed - inProgress;
  const progressPct = total > 0 ? Math.round((completed / total) * 100) : 0;

  const hasInstructions =
    project.instructions &&
    Object.values(project.instructions).some(Boolean);

  return (
    <div className="w-72 px-4 py-3 flex flex-col gap-3 overflow-y-auto shrink-0">
      {/* Progress ring + stats */}
      <section className="flex items-start gap-3">
        <div className="relative w-16 h-16 shrink-0">
          <svg viewBox="0 0 36 36" className="w-full h-full -rotate-90">
            <circle
              cx="18"
              cy="18"
              r="15.5"
              fill="none"
              stroke="rgba(255,255,255,0.08)"
              strokeWidth="3"
            />
            <circle
              cx="18"
              cy="18"
              r="15.5"
              fill="none"
              stroke={project.color}
              strokeWidth="3"
              strokeLinecap="round"
              strokeDasharray={`${progressPct} ${100 - progressPct}`}
            />
          </svg>
          <div className="absolute inset-0 flex items-center justify-center">
            <span className="text-sm font-semibold text-primary tabular-nums">
              {progressPct}%
            </span>
          </div>
        </div>
        <div className="flex-1 min-w-0 flex flex-col gap-1 pt-0.5">
          <div className="flex items-center gap-1.5">
            <span className="text-sm font-semibold text-primary tabular-nums">
              {completed}/{total}
            </span>
            <span className="text-[9px] text-dim">tasks done</span>
          </div>
          {/* Mini breakdown bar */}
          {total > 0 && (
            <div className="flex h-1 rounded-full overflow-hidden bg-white/[0.06]">
              {completed > 0 && (
                <div
                  className="h-full bg-success"
                  style={{ width: `${(completed / total) * 100}%` }}
                />
              )}
              {inProgress > 0 && (
                <div
                  className="h-full bg-brand"
                  style={{ width: `${(inProgress / total) * 100}%` }}
                />
              )}
            </div>
          )}
          <div className="flex gap-3 text-[10px] text-muted mt-0.5">
            {inProgress > 0 && <span>{inProgress} active</span>}
            {pending > 0 && <span>{pending} pending</span>}
          </div>
        </div>
      </section>

      {/* Task breakdown */}
      <section className="flex flex-col gap-1.5">
        <h4 className="text-[10px] font-medium text-dim uppercase tracking-wider">
          Tasks
        </h4>
        <StatRow
          icon={<CheckCircle2 className="w-3 h-3 text-success" />}
          label="Completed"
          value={completed}
        />
        <StatRow
          icon={<Target className="w-3 h-3 text-brand" />}
          label="In Progress"
          value={inProgress}
        />
        <StatRow
          icon={<Circle className="w-3 h-3 text-muted" />}
          label="Pending"
          value={pending}
        />
      </section>

      {/* Project info */}
      <section className="flex flex-col gap-1.5">
        <h4 className="text-[10px] font-medium text-dim uppercase tracking-wider">
          Details
        </h4>
        {project.description && (
          <p className="text-[11px] text-secondary leading-relaxed">
            {project.description}
          </p>
        )}
        {project.startDate && (
          <DetailRow
            icon={<CalendarDays className="w-3 h-3 text-muted" />}
            label="Start"
            value={formatDate(project.startDate)}
          />
        )}
        {project.targetEndDate && (
          <DetailRow
            icon={<CalendarDays className="w-3 h-3 text-muted" />}
            label="Target"
            value={formatDate(project.targetEndDate)}
          />
        )}
        <DetailRow
          icon={<ListTodo className="w-3 h-3 text-muted" />}
          label="Instructions"
          value={hasInstructions ? "Configured" : "Not set"}
        />
        <DetailRow
          icon={<Target className="w-3 h-3 text-muted" />}
          label="Role"
          value={project.userRole || "Not set"}
        />
      </section>

      {/* Recent completed tasks */}
      {completed > 0 && (
        <section className="flex flex-col gap-1.5">
          <h4 className="text-[10px] font-medium text-dim uppercase tracking-wider">
            Recently Completed
          </h4>
          {tasks
            .filter((t) => t.status === "done")
            .slice(0, 5)
            .map((t) => (
              <div
                key={t.id}
                className="flex items-center gap-2 text-[11px] text-secondary"
              >
                <CheckCircle2 className="w-3 h-3 text-success shrink-0" />
                <span className="truncate">{t.title}</span>
              </div>
            ))}
        </section>
      )}
    </div>
  );
}

function StatRow({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
}) {
  return (
    <div className="flex items-center gap-2 text-[11px]">
      {icon}
      <span className="text-secondary flex-1">{label}</span>
      <span className="text-primary font-medium tabular-nums">{value}</span>
    </div>
  );
}

function DetailRow({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="flex items-center gap-2 text-[11px]">
      {icon}
      <span className="text-muted">{label}</span>
      <span className="text-secondary ml-auto truncate max-w-[140px]">
        {value}
      </span>
    </div>
  );
}

function formatDate(iso: string): string {
  return new Date(`${iso}T00:00:00`).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}
