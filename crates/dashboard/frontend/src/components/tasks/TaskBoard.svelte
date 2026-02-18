<script lang="ts">
  interface Task {
    id: string;
    title: string;
    status: 'Todo' | 'Doing' | 'Done';
    priority?: number;
    tags?: string[];
    dueDate?: string;
    parentId?: string | null;
    projectId?: string | null;
    attachments?: any[];
    timeEntries?: any[];
    dependencies?: string[];
    enriched?: boolean;
  }

  interface Column {
    title: string;
    status: 'Todo' | 'Doing' | 'Done';
    tasks: Task[];
  }

  let {
    tasks = [],
    loading = false,
    onDragEnd,
    onTaskClick,
  } = $props<{
    tasks?: Task[];
    loading?: boolean;
    onDragEnd?: (event: { taskId: string; newStatus: string }) => void;
    onTaskClick?: (task: Task) => void;
  }>();

  const statusLabels = ['Todo', 'Doing', 'Done'] as const;

  const columns = $derived(
    statusLabels.map(status => ({
      title: status,
      status,
      tasks: tasks.filter(t => t.status === status),
    }))
  );

  let draggedTaskId = $state<string | null>(null);
  let draggedTask = $state<Task | null>(null);

  function onDragStart(task: Task) {
    draggedTaskId = task.id;
    draggedTask = task;
  }

  function onDrop(status: string) {
    if (draggedTask && draggedTask.status !== status) {
      onDragEnd?.({ taskId: draggedTaskId!, newStatus: status });
    }
    draggedTaskId = null;
    draggedTask = null;
  }

  function onDragOver(event: DragEvent) {
    event.preventDefault();
  }
</script>

<div
  class="task-board"
  role="region"
  aria-label="Task board"
  data-testid="task-board"
>
  {#each columns as column}
    <div
      class="task-column"
      role="list"
      aria-label="{column.title} tasks, {column.tasks.length} items"
      data-testid="column-{column.status.toLowerCase()}"
      data-status={column.status}
      ondragover={onDragOver}
      ondrop={() => onDrop(column.status)}
    >
      <div class="column-header">
        <h3 class="column-title">{column.title}</h3>
        <span class="column-count" aria-label="{column.tasks.length} tasks">
          {column.tasks.length}
        </span>
      </div>

      <div class="column-body">
        {#if loading}
          <div class="skeleton-card" aria-hidden="true" data-testid="skeleton"></div>
          <div class="skeleton-card" aria-hidden="true" data-testid="skeleton"></div>
        {:else if column.tasks.length === 0}
          <div class="empty-state" data-testid="empty-{column.status.toLowerCase()}">
            No {column.title.toLowerCase()} tasks
          </div>
        {:else}
          {#each column.tasks as task (task.id)}
            <div
              role="listitem"
              class="task-card-wrapper"
              draggable="true"
              ondragstart={() => onDragStart(task)}
              tabindex="0"
              aria-label="{task.title}"
              data-task-id={task.id}
              onclick={() => onTaskClick?.(task)}
              onkeydown={(e) => {
                if (e.key === ' ' || e.key === 'Enter') {
                  e.preventDefault();
                  onTaskClick?.(task);
                }
              }}
            >
              <div class="task-card-title">{task.title}</div>
              {#if task.priority}
                <span
                  class="priority-dot"
                  data-priority={task.priority}
                  aria-label="priority {task.priority}"
                ></span>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>
  {/each}
</div>

<style>
  .task-board { display: flex; gap: 16px; padding: 16px; height: 100%; overflow-x: auto; }
  .task-column { flex: 1; min-width: 280px; background: #161B22; border-radius: 12px; border: 1px solid #30363D; }
  .column-header { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid #30363D; }
  .column-title { font-size: 14px; font-weight: 600; color: #E6EDF3; margin: 0; }
  .column-count { font-size: 12px; color: #8B949E; background: #21262D; padding: 2px 8px; border-radius: 9999px; }
  .column-body { padding: 8px; display: flex; flex-direction: column; gap: 8px; }
  .task-card-wrapper { background: #1C2128; border: 1px solid #30363D; border-radius: 8px; padding: 12px; cursor: pointer; position: relative; }
  .task-card-title { font-size: 14px; color: #E6EDF3; }
  .priority-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; }
  .priority-dot[data-priority="1"] { background: #F85149; }
  .priority-dot[data-priority="2"] { background: #FF8C00; }
  .priority-dot[data-priority="3"] { background: #D29922; }
  .priority-dot[data-priority="4"] { background: #8B949E; }
  .skeleton-card { height: 80px; background: linear-gradient(90deg, #21262D 25%, #2D333B 50%, #21262D 75%); border-radius: 8px; animation: shimmer 1.5s infinite; }
  .empty-state { color: #6E7681; font-size: 14px; text-align: center; padding: 24px; }
  @keyframes shimmer { 0% { background-position: -200% 0; } 100% { background-position: 200% 0; } }
</style>
