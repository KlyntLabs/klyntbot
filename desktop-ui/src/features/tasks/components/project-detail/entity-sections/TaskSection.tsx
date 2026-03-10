import { CollapsibleSection } from "@shared/components";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { Task } from "@shared/types";
import { Badge, Checkbox } from "@shared/ui";
import { CheckSquare } from "lucide-react";
import { useNavigate } from "react-router";

interface TaskSectionProps {
  projectId: string;
  defaultOpen?: boolean;
}

export function TaskSection({ projectId, defaultOpen }: TaskSectionProps) {
  const navigate = useNavigate();
  const { data: tasks, refetch } = useQuery<Task[]>("task_list", { project_id: projectId }, []);
  const toggleComplete = useMutation<Task, { id: string }>("task_toggle_complete");

  const handleToggle = async (id: string) => {
    await toggleComplete.mutate({ id });
    refetch();
  };

  const completedCount = tasks.filter((t) => t.completed).length;

  return (
    <CollapsibleSection
      title="Tasks"
      icon={<CheckSquare className="w-3.5 h-3.5 text-brand" strokeWidth={1.5} />}
      count={tasks.length || null}
      defaultOpen={defaultOpen}
    >
      {tasks.length === 0 ? (
        <p className="text-[11px] text-dim font-light py-2">No tasks</p>
      ) : (
        <>
          <p className="text-[10px] text-dim mb-1.5">
            {completedCount}/{tasks.length} complete
          </p>
          <div className="space-y-0.5 max-h-64 overflow-y-auto">
            {tasks.map((task) => (
              <div
                key={task.id}
                className="flex items-center gap-2 px-2 py-1 rounded-md hover:bg-white/[0.04] transition-colors cursor-pointer"
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
                  <Checkbox
                    checked={task.completed}
                    onCheckedChange={() => handleToggle(task.id)}
                  />
                </fieldset>
                <span
                  className={`text-[11px] font-light truncate flex-1 ${task.completed ? "text-muted line-through" : "text-secondary"}`}
                >
                  {task.title}
                </span>
                {task.priority && <Badge variant="brand">{task.priority}</Badge>}
              </div>
            ))}
          </div>
        </>
      )}
    </CollapsibleSection>
  );
}
