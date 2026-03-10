import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { Task } from "@shared/types";
import { Checkbox } from "@shared/ui";
import { useNavigate } from "react-router";

interface TasksColumnProps {
  projectId: string;
}

const PRIORITY_COLORS: Record<string, string> = {
  "1": "#ef4444",
  "2": "#f97316",
  "3": "#3b82f6",
};

export function TasksColumn({ projectId }: TasksColumnProps) {
  const navigate = useNavigate();
  const { data: tasks, refetch } = useQuery<Task[]>("task_list", { project_id: projectId }, []);
  const toggleComplete = useMutation<Task, { id: string }>("task_toggle_complete");

  const handleToggle = async (id: string) => {
    await toggleComplete.mutate({ id });
    refetch();
  };

  if (tasks.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-[11px] text-dim font-light">
        No tasks
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0.5 p-2">
      {tasks.map((task) => (
        <div
          key={task.id}
          className="flex items-center gap-2 px-2.5 py-1.5 rounded-md hover:bg-white/[0.04] transition-colors cursor-pointer"
          role="button"
          tabIndex={0}
          onClick={() => navigate(`/task/${task.id}`)}
          onKeyDown={(e) => {
            if (e.key === "Enter") navigate(`/task/${task.id}`);
          }}
        >
          <fieldset
            className="border-none p-0 m-0"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <Checkbox checked={task.completed} onCheckedChange={() => handleToggle(task.id)} />
          </fieldset>
          {task.priority && (
            <div
              className="w-1.5 h-1.5 rounded-full shrink-0"
              style={{ backgroundColor: PRIORITY_COLORS[task.priority] ?? "#6b7280" }}
            />
          )}
          <span
            className={`text-[11px] font-light truncate ${task.completed ? "text-muted line-through" : "text-secondary"}`}
          >
            {task.title}
          </span>
        </div>
      ))}
    </div>
  );
}
