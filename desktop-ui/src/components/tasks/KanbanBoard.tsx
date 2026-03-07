import {
  closestCorners,
  DndContext,
  type DragEndEvent,
  type DragOverEvent,
  DragOverlay,
  type DragStartEvent,
  PointerSensor,
  useDroppable,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useCallback, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { formatDate } from "../../lib/dates";
import type { Area, Project, StatusLabel, Task, TaskUpdateParams } from "../../lib/types";
import { Badge } from "../ui/Badge";

interface KanbanBoardProps {
  tasks: Task[];
  projectMap: Map<string, Project>;
  areaMap: Map<string, Area>;
  completedTasks: Set<string>;
  statusLabels: StatusLabel[];
  onUpdateTask: (params: TaskUpdateParams) => Promise<void>;
  onRefetch: () => void;
}

interface ColumnData {
  key: string;
  label: string;
  color: string;
  tasks: Task[];
}

// ── Card content (shared between inline card and drag overlay) ──────

function CardContent({
  task,
  projectMap,
  areaMap,
  isCompleted,
}: {
  task: Task;
  projectMap: Map<string, Project>;
  areaMap: Map<string, Area>;
  isCompleted: boolean;
}) {
  const project = task.projectId ? projectMap.get(task.projectId) : undefined;
  const area = areaMap.get(task.areaId);

  return (
    <>
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

      {/* Subtask progress */}
      {task.subtaskCount > 0 && (
        <div className="flex items-center gap-1.5 mt-2">
          <div className="w-12 h-1 rounded-full bg-white/[0.08] overflow-hidden">
            <div
              className="h-full rounded-full bg-brand"
              style={{
                width: `${(task.subtaskCompletedCount / task.subtaskCount) * 100}%`,
              }}
            />
          </div>
          <span className="text-[10px] text-dim font-light">
            {task.subtaskCompletedCount}/{task.subtaskCount}
          </span>
        </div>
      )}

      {/* Due date */}
      {task.dueDate && (
        <p className="text-[10px] text-dim font-light mt-2">{formatDate(task.dueDate)}</p>
      )}
    </>
  );
}

// ── Sortable card ───────────────────────────────────────────────────

function SortableKanbanCard({
  task,
  projectMap,
  areaMap,
  isCompleted,
  onNavigate,
  dragJustEnded,
}: {
  task: Task;
  projectMap: Map<string, Project>;
  areaMap: Map<string, Area>;
  isCompleted: boolean;
  onNavigate: (id: string) => void;
  dragJustEnded: React.RefObject<boolean>;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: task.id,
    data: { type: "card", task },
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.3 : 1,
  };

  return (
    <button
      ref={setNodeRef}
      type="button"
      style={style}
      {...attributes}
      {...listeners}
      onClick={() => {
        if (!isDragging && !dragJustEnded.current) onNavigate(task.id);
      }}
      className="bg-white/[0.04] hover:bg-white/[0.06] rounded-lg px-4 py-3 cursor-grab active:cursor-grabbing transition-colors border border-white/[0.04] text-left w-full"
    >
      <CardContent
        task={task}
        projectMap={projectMap}
        areaMap={areaMap}
        isCompleted={isCompleted}
      />
    </button>
  );
}

// ── Droppable column wrapper ────────────────────────────────────────

function DroppableColumn({
  columnId,
  children,
  isOver,
}: {
  columnId: string;
  children: React.ReactNode;
  isOver: boolean;
}) {
  const { setNodeRef } = useDroppable({
    id: `column-${columnId}`,
    data: { type: "column", columnId },
  });

  return (
    <div
      ref={setNodeRef}
      className={`flex-1 overflow-y-auto space-y-2 pr-0.5 min-h-[60px] rounded-lg transition-colors ${
        isOver ? "bg-white/[0.02] ring-1 ring-white/[0.06]" : ""
      }`}
    >
      {children}
    </div>
  );
}

// ── Main KanbanBoard ────────────────────────────────────────────────

export function KanbanBoard({
  tasks,
  projectMap,
  areaMap,
  completedTasks,
  statusLabels,
  onUpdateTask,
  onRefetch,
}: KanbanBoardProps) {
  const navigate = useNavigate();
  const [activeTask, setActiveTask] = useState<Task | null>(null);
  const [overColumnId, setOverColumnId] = useState<string | null>(null);
  const dragJustEnded = useRef(false);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 8 },
    }),
  );

  const columns = useMemo(() => {
    const cols: ColumnData[] = statusLabels.map((sl) => ({
      key: sl.id,
      label: sl.name,
      color: sl.color,
      tasks: [] as Task[],
    }));

    const colMap = new Map(cols.map((c) => [c.key, c]));

    for (const task of tasks) {
      const col = task.statusLabelId ? colMap.get(task.statusLabelId) : undefined;
      if (col) {
        col.tasks.push(task);
      } else if (cols.length > 0) {
        cols[0].tasks.push(task);
      }
    }

    return cols;
  }, [tasks, statusLabels]);

  // Build a task lookup
  const taskMap = useMemo(() => new Map(tasks.map((t) => [t.id, t])), [tasks]);

  // Find which column a task belongs to
  const findColumnForTask = useCallback(
    (taskId: string): string | null => {
      for (const col of columns) {
        if (col.tasks.some((t) => t.id === taskId)) {
          return col.key;
        }
      }
      return null;
    },
    [columns],
  );

  const handleDragStart = useCallback(
    (event: DragStartEvent) => {
      const task = taskMap.get(event.active.id as string);
      if (task) setActiveTask(task);
    },
    [taskMap],
  );

  const handleDragOver = useCallback(
    (event: DragOverEvent) => {
      const { over } = event;
      if (!over) {
        setOverColumnId(null);
        return;
      }

      // Determine which column is being hovered
      const overData = over.data.current;
      if (overData?.type === "column") {
        setOverColumnId(overData.columnId as string);
      } else if (overData?.type === "card") {
        const colId = findColumnForTask(over.id as string);
        setOverColumnId(colId);
      } else {
        setOverColumnId(null);
      }
    },
    [findColumnForTask],
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      setActiveTask(null);
      setOverColumnId(null);

      // Suppress click events that fire immediately after drag release
      dragJustEnded.current = true;
      requestAnimationFrame(() => {
        dragJustEnded.current = false;
      });

      const { active, over } = event;
      if (!over) return;

      const activeId = active.id as string;
      const overData = over.data.current;

      const sourceColumnId = findColumnForTask(activeId);
      if (!sourceColumnId) return;

      let targetColumnId: string;
      let targetTaskId: string | null = null;

      if (overData?.type === "column") {
        targetColumnId = overData.columnId as string;
      } else if (overData?.type === "card") {
        targetTaskId = over.id as string;
        const col = findColumnForTask(targetTaskId);
        targetColumnId = col ?? sourceColumnId;
      } else {
        return;
      }

      try {
        // Moving between columns
        if (sourceColumnId !== targetColumnId) {
          const targetCol = columns.find((c) => c.key === targetColumnId);
          let newPosition = 0;
          if (targetCol && targetTaskId) {
            const targetIdx = targetCol.tasks.findIndex((t) => t.id === targetTaskId);
            newPosition = targetIdx >= 0 ? targetIdx : targetCol.tasks.length;
          } else if (targetCol) {
            newPosition = targetCol.tasks.length;
          }

          await onUpdateTask({
            id: activeId,
            statusLabelId: targetColumnId,
            position: newPosition,
          });
          return;
        }

        // Reordering within the same column
        if (targetTaskId && targetTaskId !== activeId) {
          const col = columns.find((c) => c.key === sourceColumnId);
          if (!col) return;

          const oldIndex = col.tasks.findIndex((t) => t.id === activeId);
          const newIndex = col.tasks.findIndex((t) => t.id === targetTaskId);

          if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
            const reordered = arrayMove(col.tasks, oldIndex, newIndex);
            const finalIndex = reordered.findIndex((t) => t.id === activeId);

            await onUpdateTask({
              id: activeId,
              position: finalIndex,
            });
          }
        }
      } catch (e) {
        console.error("Failed to update task position:", e);
      } finally {
        onRefetch();
      }
    },
    [columns, findColumnForTask, onUpdateTask, onRefetch, dragJustEnded],
  );

  const handleNavigate = useCallback(
    (id: string) => {
      navigate(`/task/${id}`);
    },
    [navigate],
  );

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCorners}
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDragEnd={handleDragEnd}
    >
      <div className="flex gap-3 mb-10 min-h-0">
        {columns.map((col) => {
          const taskIds = col.tasks.map((t) => t.id);

          return (
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
              <SortableContext items={taskIds} strategy={verticalListSortingStrategy}>
                <DroppableColumn columnId={col.key} isOver={overColumnId === col.key}>
                  {col.tasks.map((task) => (
                    <SortableKanbanCard
                      key={task.id}
                      task={task}
                      projectMap={projectMap}
                      areaMap={areaMap}
                      isCompleted={completedTasks.has(task.id)}
                      onNavigate={handleNavigate}
                      dragJustEnded={dragJustEnded}
                    />
                  ))}

                  {col.tasks.length === 0 && (
                    <div className="flex items-center justify-center py-8">
                      <div className="border-2 border-dashed border-white/10 rounded-lg h-16 w-full flex items-center justify-center">
                        <p className="text-[11px] text-dim font-light">No tasks</p>
                      </div>
                    </div>
                  )}
                </DroppableColumn>
              </SortableContext>
            </div>
          );
        })}
      </div>

      {/* Drag overlay — renders the floating ghost card */}
      <DragOverlay>
        {activeTask ? (
          <div className="bg-white/[0.06] rounded-lg px-4 py-3 border border-white/[0.08] shadow-lg opacity-90 w-[240px]">
            <CardContent
              task={activeTask}
              projectMap={projectMap}
              areaMap={areaMap}
              isCompleted={completedTasks.has(activeTask.id)}
            />
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}
