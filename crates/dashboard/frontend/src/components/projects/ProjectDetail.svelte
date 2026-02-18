<script lang="ts">
  interface Task {
    id: string;
    title: string;
    status: string;
    priority?: number;
    parentId?: string | null;
    timeEntries?: Array<{ id: string; minutes: number }>;
  }

  interface Project {
    id: string;
    name: string;
    description?: string;
    status: 'Active' | 'Paused' | 'Completed' | 'Archived';
    color: 'red' | 'orange' | 'yellow' | 'green' | 'blue' | 'purple' | 'gray';
    tags?: string[];
  }

  let {
    project,
    tasks = [],
    onArchive,
    onEdit,
    onAddTask,
  }: {
    project: Project;
    tasks?: Task[];
    onArchive?: (id: string) => void;
    onEdit?: (project: Project) => void;
    onAddTask?: () => void;
  } = $props();

  const totalTasks = $derived(tasks.length);
  const completedTasks = $derived(tasks.filter(t => t.status === 'Done').length);
  const completionPct = $derived(totalTasks > 0 ? Math.round((completedTasks / totalTasks) * 100) : 0);
  const timeTracked = $derived(
    tasks.reduce((sum, t) => sum + (t.timeEntries?.reduce((s, e) => s + e.minutes, 0) ?? 0), 0)
  );

  function formatMinutes(mins: number) {
    if (mins < 60) return `${mins}m`;
    return `${Math.floor(mins / 60)}h ${mins % 60}m`;
  }
</script>

<div class="project-detail">
  <header class="project-header">
    <h1>{project.name}</h1>
    {#if project.description}
      <p class="description">{project.description}</p>
    {/if}
    <div class="stats" data-testid="project-stats">
      <div class="stat">
        <span class="stat-value">{totalTasks}</span>
        <span class="stat-label">Total Tasks</span>
      </div>
      <div class="stat">
        <span class="stat-value">{completionPct}%</span>
        <span class="stat-label">Complete</span>
      </div>
      <div class="stat">
        <span class="stat-value">{formatMinutes(timeTracked)}</span>
        <span class="stat-label">Time Tracked</span>
      </div>
    </div>
    <div class="actions">
      <button type="button" onclick={() => onEdit?.(project)}>Edit</button>
      {#if project.status !== 'Archived'}
        <button
          type="button"
          class="archive-btn"
          onclick={() => onArchive?.(project.id)}
        >
          Archive
        </button>
      {/if}
      <button type="button" class="add-task-btn" onclick={() => onAddTask?.()}>+ Add Task</button>
    </div>
  </header>

  <section class="task-tree-section" data-testid="task-tree">
    {#if tasks.length === 0}
      <p class="empty">No tasks in this project</p>
    {:else}
      <ul class="task-tree">
        {#each tasks.filter(t => !t.parentId) as task (task.id)}
          {@const subtasks = tasks.filter(t => t.parentId === task.id)}
          <li class="task-item">
            <span class="task-title">{task.title}</span>
            {#if subtasks.length > 0}
              <ul class="subtask-list">
                {#each subtasks as sub (sub.id)}
                  <li class="task-item subtask">
                    <span class="task-title">{sub.title}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .project-detail {
    padding: 24px;
  }
  .project-header {
    margin-bottom: 32px;
  }
  h1 {
    font-size: 24px;
    font-weight: 700;
    color: #E6EDF3;
    margin: 0 0 8px;
  }
  .description {
    color: #8B949E;
    margin: 0 0 16px;
  }
  .stats {
    display: flex;
    gap: 24px;
    margin-bottom: 16px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .stat-value {
    font-size: 20px;
    font-weight: 600;
    color: #E6EDF3;
  }
  .stat-label {
    font-size: 12px;
    color: #8B949E;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  button {
    background: #21262D;
    color: #E6EDF3;
    border: 1px solid #30363D;
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 14px;
    cursor: pointer;
  }
  .archive-btn {
    color: #D29922;
    border-color: #D29922;
  }
  .add-task-btn {
    background: #FF8C00;
    color: #0D1117;
    border-color: #FF8C00;
    font-weight: 600;
  }
  .empty {
    color: #6E7681;
    padding: 24px 0;
  }
  .task-tree {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .task-item {
    padding: 8px 0;
    border-bottom: 1px solid #21262D;
    color: #E6EDF3;
  }
  .subtask-list {
    list-style: none;
    padding-left: 24px;
    margin: 4px 0 0;
  }
  .subtask {
    font-size: 13px;
    color: #8B949E;
  }
</style>
