import { useMutation } from "@shared/hooks/useMutation";
import type { Task } from "@shared/types";
import { CheckCircle2, Circle } from "lucide-react";
import { useMemo } from "react";
import { useProjectTasks } from "../../hooks/useProjectTasks";

interface LinkedTasksListProps {
  keyResultId: string;
  projectId: string;
}

export function LinkedTasksList({ keyResultId, projectId }: LinkedTasksListProps) {
  const { data: tasks } = useProjectTasks(projectId);

  const linkedTasks = useMemo(() => {
    return tasks.filter((t: Task) => {
      // Tasks linked via metadata.keyResultId or via objectiveId mapping
      const meta = t as Task & { metadata?: Record<string, unknown> };
      return meta.metadata?.keyResultId === keyResultId;
    });
  }, [tasks, keyResultId]);

  const { mutate: toggleComplete } = useMutation<void, { id: string }>("task_toggle_complete");

  const handleToggle = async (task: Task) => {
    await toggleComplete({ id: task.id });
  };

  if (linkedTasks.length === 0) {
    return (
      <div className="ml-10 px-3 py-2 text-[11px] text-muted-foreground italic">
        No linked tasks. Use "Link to KR" on a task to connect it.
      </div>
    );
  }

  return (
    <div className="ml-10 space-y-0.5 pb-1">
      {linkedTasks.map((task) => (
        <div
          key={task.id}
          className="flex items-center gap-2 px-3 py-1.5 hover:bg-accent/30 rounded transition-colors"
        >
          <button type="button" onClick={() => handleToggle(task)} className="flex-shrink-0">
            {task.completed ? (
              <CheckCircle2 className="w-3.5 h-3.5 text-brand" />
            ) : (
              <Circle className="w-3.5 h-3.5 text-muted-foreground" />
            )}
          </button>
          <span
            className={`text-[11px] truncate ${task.completed ? "line-through text-muted-foreground" : "text-foreground"}`}
          >
            {task.title}
          </span>
          <span className="ml-auto text-[10px] text-muted-foreground">{task.status}</span>
        </div>
      ))}
    </div>
  );
}
