import { useMemo } from "react";
import { useNavigate } from "react-router";
import { formatDate } from "../../lib/dates";
import type { Area, Project, StatusLabel, Task } from "../../lib/types";
import { Badge } from "../ui/Badge";

interface KanbanBoardProps {
  tasks: Task[];
  projectMap: Map<string, Project>;
  areaMap: Map<string, Area>;
  completedTasks: Set<string>;
  statusLabels: StatusLabel[];
}

export function KanbanBoard({
  tasks,
  projectMap,
  areaMap,
  completedTasks,
  statusLabels,
}: KanbanBoardProps) {
  const navigate = useNavigate();

  const columns = useMemo(() => {
    // Build column structure from labels
    const cols = statusLabels.map((sl) => ({
      key: sl.id,
      label: sl.name,
      color: sl.color,
      tasks: [] as Task[],
    }));

    // Create a map for fast lookup
    const colMap = new Map(cols.map((c) => [c.key, c]));

    // Group tasks by statusLabelId
    for (const task of tasks) {
      const col = task.statusLabelId ? colMap.get(task.statusLabelId) : undefined;
      if (col) {
        col.tasks.push(task);
      } else if (cols.length > 0) {
        // Fallback: put in first column
        cols[0].tasks.push(task);
      }
    }

    return cols;
  }, [tasks, statusLabels]);

  return (
    <div className="flex gap-3 mb-10 min-h-0">
      {columns.map((col) => (
        <div key={col.key} className="flex-1 min-w-[240px] flex flex-col min-h-0">
          {/* Column header */}
          <div className="flex items-center gap-2.5 px-3 py-2.5 mb-2">
            <div
              className="w-1.5 h-1.5 rounded-full flex-shrink-0"
              style={{ backgroundColor: col.color }}
            />
            <span className="text-[12px] font-light text-secondary">{col.label}</span>
            <span className="text-[11px] font-light text-dim">{col.tasks.length}</span>
          </div>

          {/* Cards container */}
          <div className="flex-1 overflow-y-auto space-y-2 pr-0.5">
            {col.tasks.map((task) => {
              const project = task.projectId ? projectMap.get(task.projectId) : undefined;
              const area = areaMap.get(task.areaId);
              const isCompleted = completedTasks.has(task.id);

              return (
                <button
                  type="button"
                  key={task.id}
                  onClick={() => navigate(`/task/${task.id}`)}
                  className="bg-white/[0.04] hover:bg-white/[0.06] rounded-lg px-4 py-3 cursor-pointer transition-colors border border-white/[0.04] text-left w-full"
                >
                  {/* Title */}
                  <p
                    className={`text-[13px] font-light leading-snug mb-2 ${
                      isCompleted ? "text-muted line-through" : "text-secondary"
                    }`}
                  >
                    {task.title}
                  </p>

                  {/* Meta row */}
                  <div className="flex items-center gap-1.5 flex-wrap">
                    {task.priority && <Badge variant="priority" value={task.priority} />}
                    {project && (
                      <div className="flex items-center gap-1.5 px-1.5 py-0.5 rounded bg-white/[0.06]">
                        <div
                          className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                          style={{ backgroundColor: project.color }}
                        />
                        <span className="text-[10px] font-light text-muted">{project.name}</span>
                      </div>
                    )}
                    {area && <Badge variant="area" value={area.name} />}
                  </div>

                  {/* Tags */}
                  {task.tags.length > 0 && (
                    <div className="flex items-center gap-1 mt-2 flex-wrap">
                      {task.tags.map((tag) => (
                        <Badge key={tag} variant="tag" value={tag} />
                      ))}
                    </div>
                  )}

                  {/* Due date */}
                  {task.dueDate && (
                    <p className="text-[10px] text-dim font-light mt-2">
                      {formatDate(task.dueDate)}
                    </p>
                  )}
                </button>
              );
            })}

            {col.tasks.length === 0 && (
              <div className="flex items-center justify-center py-8">
                <p className="text-[11px] text-dim font-light">No tasks</p>
              </div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
