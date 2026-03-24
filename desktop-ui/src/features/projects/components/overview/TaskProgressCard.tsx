import { todayISO } from "@shared/lib/dates";
import { Progress } from "@shared/ui";
import { useMemo } from "react";
import { useNavigate } from "react-router";
import { useProjectContext } from "../../contexts/ProjectContext";

export function TaskProgressCard() {
  const navigate = useNavigate();
  const { project, tasks } = useProjectContext();

  const totalCount = project?.taskCount ?? 0;
  const completedCount = project?.completedCount ?? 0;
  const activeCount = Math.max(0, totalCount - completedCount);
  const progressPercent = totalCount > 0 ? Math.round((completedCount / totalCount) * 100) : 0;

  const { dueToday, overdue } = useMemo(() => {
    const todayStr = todayISO();
    let dueTodayCount = 0;
    let overdueCount = 0;
    for (const t of tasks) {
      if (t.completed || !t.dueDate) continue;
      const due = t.dueDate.slice(0, 10);
      if (due === todayStr) dueTodayCount++;
      else if (due < todayStr) overdueCount++;
    }
    return { dueToday: dueTodayCount, overdue: overdueCount };
  }, [tasks]);

  return (
    <button
      type="button"
      onClick={() => navigate(`/project/${project?.id ?? ""}/tasks`)}
      className="glass-card rounded-xl p-5 text-left transition-colors hover:border-brand/30"
    >
      <p className="text-2xs text-muted-foreground uppercase tracking-wider mb-3">Task Progress</p>
      <span className="text-2xl font-bold text-foreground">{activeCount}</span>
      <span className="text-[11px] text-muted-foreground ml-1.5">
        active task{activeCount !== 1 ? "s" : ""}
      </span>

      <div className="mt-3 mb-2">
        <Progress
          value={progressPercent}
          color={progressPercent >= 80 ? "success" : progressPercent >= 50 ? "brand" : "warning"}
        />
      </div>

      <div className="flex items-center gap-3 text-[11px] text-muted-foreground">
        <span>
          {completedCount}/{totalCount} completed
        </span>
        {dueToday > 0 && <span className="text-warning">{dueToday} due today</span>}
        {overdue > 0 && <span className="text-destructive">{overdue} overdue</span>}
      </div>
    </button>
  );
}
