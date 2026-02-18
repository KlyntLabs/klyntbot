<script lang="ts">
  interface TimeEntry {
    id: string;
    minutes: number;
    note?: string;
  }

  interface Attachment {
    id: string;
    type: 'file' | 'url' | 'note';
    title: string;
  }

  interface Task {
    id: string;
    title: string;
    status: 'Todo' | 'Doing' | 'Done';
    priority?: number;
    tags?: string[];
    dueDate?: string;
    parentId?: string | null;
    projectId?: string | null;
    attachments?: Attachment[];
    timeEntries?: TimeEntry[];
    dependencies?: string[];
    enriched?: boolean;
    subtaskCount?: number;
    completedSubtasks?: number;
    focusActive?: boolean;
    focusElapsed?: number;
  }

  const priorityNames = ['', 'critical', 'high', 'medium', 'low'];

  let {
    task,
    compact = false,
    draggable = true,
    onClick,
  } = $props<{
    task: Task;
    compact?: boolean;
    draggable?: boolean;
    onClick?: (task: Task) => void;
  }>();

  const priorityName = $derived(
    task.priority ? priorityNames[task.priority] || 'medium' : 'medium'
  );

  const ariaLabel = $derived.by(() => {
    const parts = [task.title];
    if (task.priority) parts.push(`priority ${priorityName}`);
    if (task.dueDate) {
      const date = new Date(task.dueDate);
      const today = new Date();
      const tomorrow = new Date(today);
      tomorrow.setDate(today.getDate() + 1);
      if (date.toDateString() === today.toDateString()) parts.push('due today');
      else if (date.toDateString() === tomorrow.toDateString()) parts.push('due tomorrow');
      else parts.push(`due ${date.toLocaleDateString()}`);
    }
    return parts.join(', ');
  });

  function formatElapsed(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
</script>

<div
  class="task-card"
  class:compact
  role="listitem"
  aria-label={ariaLabel}
  data-testid="task-card"
  data-task-id={task.id}
  tabindex="0"
  onclick={() => onClick?.(task)}
  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onClick?.(task); } }}
>
  <div class="card-header">
    {#if task.priority}
      <span
        class="priority-dot"
        class:critical={task.priority === 1}
        class:high={task.priority === 2}
        class:medium={task.priority === 3}
        class:low={task.priority === 4}
        aria-label="Priority: {priorityName}"
        data-testid="priority-dot"
      ></span>
    {/if}
    <span class="task-title" data-testid="task-title">{task.title}</span>
    {#if task.enriched}
      <span class="enriched-badge" data-testid="enriched-badge">AI enriched</span>
    {/if}
  </div>

  {#if !compact}
    {#if task.tags && task.tags.length > 0}
      <div class="task-tags" data-testid="task-tags">
        {#each task.tags as tag}
          <span class="tag">{tag}</span>
        {/each}
      </div>
    {/if}

    {#if task.dueDate}
      <div class="due-date" data-testid="due-date">
        Due: {new Date(task.dueDate).toLocaleDateString()}
      </div>
    {/if}

    {#if task.subtaskCount != null && task.subtaskCount > 0}
      <div class="subtask-progress" data-testid="subtask-progress">
        {task.completedSubtasks ?? 0}/{task.subtaskCount} subtasks
      </div>
    {/if}

    {#if task.focusActive}
      <div class="focus-indicator" data-testid="focus-indicator">
        ⏱ {formatElapsed(task.focusElapsed ?? 0)}
      </div>
    {/if}
  {/if}
</div>

<style>
  .task-card { background: #1C2128; border: 1px solid #30363D; border-radius: 8px; padding: 12px; cursor: pointer; display: flex; flex-direction: column; gap: 8px; transition: background 150ms ease; }
  .task-card:hover { background: #21262D; }
  .card-header { display: flex; align-items: center; gap: 8px; }
  .priority-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .priority-dot.critical { background: #F85149; }
  .priority-dot.high { background: #FF8C00; }
  .priority-dot.medium { background: #D29922; }
  .priority-dot.low { background: #8B949E; }
  .task-title { font-size: 14px; color: #E6EDF3; flex: 1; }
  .enriched-badge { font-size: 10px; background: rgba(88,166,255,0.15); color: #58A6FF; padding: 1px 6px; border-radius: 9999px; }
  .task-tags { display: flex; flex-wrap: wrap; gap: 4px; }
  .tag { font-size: 11px; background: #21262D; color: #8B949E; padding: 2px 6px; border-radius: 4px; }
  .due-date { font-size: 12px; color: #8B949E; }
  .subtask-progress { font-size: 12px; color: #8B949E; }
  .focus-indicator { font-size: 12px; color: #FF8C00; }
  .compact .task-tags, .compact .due-date, .compact .subtask-progress { display: none; }
</style>
