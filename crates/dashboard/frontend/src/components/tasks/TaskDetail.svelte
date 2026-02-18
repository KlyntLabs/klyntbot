<script lang="ts">
  interface Attachment {
    id: string;
    type: 'file' | 'url' | 'note';
    title: string;
    path?: string;
    url?: string;
    content?: string;
  }

  interface TimeEntry {
    id: string;
    minutes: number;
    note?: string;
    createdAt: string;
  }

  interface Task {
    id: string;
    title: string;
    description?: string;
    status: 'Todo' | 'Doing' | 'Done';
    priority?: number;
    tags?: string[];
    dueDate?: string;
    parentId?: string | null;
    projectId?: string | null;
    attachments?: Attachment[];
    timeEntries?: TimeEntry[];
    dependencies?: string[];
    subtasks?: Task[];
  }

  let {
    task,
    onUpdate,
    onClose,
    allTasks = [],
  } = $props<{
    task: Task;
    onUpdate?: (update: Partial<Task>) => void;
    onClose?: () => void;
    allTasks?: Task[];
  }>();

  let editingTitle = $state(false);
  let titleValue = $state(task.title);
  let showSubtaskForm = $state(false);
  let newSubtaskTitle = $state('');

  const totalMinutes = $derived(
    (task.timeEntries ?? []).reduce((sum, e) => sum + e.minutes, 0)
  );

  const subtasks = $derived(task.subtasks ?? []);
  const completedSubtasks = $derived(subtasks.filter(s => s.status === 'Done').length);

  function startEditTitle() {
    titleValue = task.title;
    editingTitle = true;
  }

  function saveTitle(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      onUpdate?.({ title: titleValue });
      editingTitle = false;
    } else if (e.key === 'Escape') {
      titleValue = task.title;
      editingTitle = false;
    }
  }

  function addSubtask(e: KeyboardEvent) {
    if (e.key === 'Enter' && newSubtaskTitle.trim()) {
      onUpdate?.({ title: newSubtaskTitle.trim() });
      newSubtaskTitle = '';
      showSubtaskForm = false;
    } else if (e.key === 'Escape') {
      newSubtaskTitle = '';
      showSubtaskForm = false;
    }
  }

  function formatMinutes(minutes: number): string {
    if (minutes < 60) return `${minutes}m`;
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    return m > 0 ? `${h}h ${m}m` : `${h}h`;
  }

  const statusOptions = ['Todo', 'Doing', 'Done'] as const;
</script>

<aside
  class="task-detail"
  data-testid="task-detail"
  role="complementary"
  aria-label="Task detail"
>
  <header class="detail-header">
    <button class="back-btn" onclick={onClose} aria-label="Close task detail" data-testid="close-btn">
      ← Back
    </button>
    <h2 class="detail-title" data-testid="task-title-display">
      {#if editingTitle}
        <input
          class="title-input"
          bind:value={titleValue}
          onkeydown={saveTitle}
          autofocus
          data-testid="title-input"
        />
      {:else}
        <span
          class="title-text"
          role="button"
          tabindex="0"
          onclick={startEditTitle}
          onkeydown={(e) => { if (e.key === 'Enter') startEditTitle(); }}
          data-testid="title-text"
        >{task.title}</span>
      {/if}
    </h2>
  </header>

  <div class="detail-body">
    <!-- Status & Priority -->
    <div class="field-row">
      <label for="status-select" class="field-label">Status</label>
      <select
        id="status-select"
        class="field-select"
        value={task.status}
        onchange={(e) => onUpdate?.({ status: (e.target as HTMLSelectElement).value as Task['status'] })}
        data-testid="status-select"
      >
        {#each statusOptions as s}
          <option value={s}>{s}</option>
        {/each}
      </select>
    </div>

    {#if task.priority}
    <div class="field-row">
      <label class="field-label">Priority</label>
      <span class="priority-badge" data-testid="priority-badge">
        {['', 'Critical', 'High', 'Medium', 'Low'][task.priority] ?? 'Medium'}
      </span>
    </div>
    {/if}

    {#if task.dueDate}
    <div class="field-row">
      <label class="field-label">Due</label>
      <span data-testid="due-date">{new Date(task.dueDate).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}</span>
    </div>
    {/if}

    {#if task.tags && task.tags.length > 0}
    <div class="field-row">
      <label class="field-label">Tags</label>
      <div class="tags-list" data-testid="tags-list">
        {#each task.tags as tag}
          <span class="tag">{tag}</span>
        {/each}
      </div>
    </div>
    {/if}

    {#if task.description}
    <div class="field-section">
      <label class="section-label">Description</label>
      <p class="description-text" data-testid="description">{task.description}</p>
    </div>
    {/if}

    <!-- Subtasks -->
    <div class="field-section" data-testid="subtasks-section">
      <div class="section-header">
        <label class="section-label">
          Subtasks
          {#if subtasks.length > 0}
            <span class="progress-text" data-testid="subtask-progress">
              ({completedSubtasks}/{subtasks.length} done)
            </span>
          {/if}
        </label>
        <button class="add-btn" onclick={() => (showSubtaskForm = true)} data-testid="add-subtask-btn">
          + Add Subtask
        </button>
      </div>

      {#if showSubtaskForm}
        <input
          class="inline-input"
          bind:value={newSubtaskTitle}
          placeholder="Subtask title..."
          onkeydown={addSubtask}
          autofocus
          data-testid="subtask-input"
        />
      {/if}

      {#if subtasks.length > 0}
        <ul class="subtask-list" data-testid="subtask-list">
          {#each subtasks as sub}
            <li class="subtask-item" data-testid="subtask-item">
              <input type="checkbox" checked={sub.status === 'Done'} readonly />
              <span>{sub.title}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Time tracking -->
    {#if (task.timeEntries ?? []).length > 0}
    <div class="field-section" data-testid="time-section">
      <label class="section-label">Time Tracked: {formatMinutes(totalMinutes)}</label>
      <ul class="time-entries">
        {#each task.timeEntries ?? [] as entry}
          <li>{formatMinutes(entry.minutes)}{entry.note ? ` — ${entry.note}` : ''}</li>
        {/each}
      </ul>
    </div>
    {/if}

    <!-- Attachments -->
    {#if (task.attachments ?? []).length > 0}
    <div class="field-section" data-testid="attachments-section">
      <div class="section-header">
        <label class="section-label">Attachments</label>
        <button class="add-btn">+ Add</button>
      </div>
      <ul class="attachment-list" data-testid="attachment-list">
        {#each task.attachments ?? [] as att}
          <li class="attachment-item" data-testid="attachment-item">
            📎 {att.title}
          </li>
        {/each}
      </ul>
    </div>
    {/if}

    <!-- Dependencies -->
    {#if (task.dependencies ?? []).length > 0}
    <div class="field-section" data-testid="dependencies-section">
      <label class="section-label">Dependencies</label>
      <div data-testid="dependencies-list">
        {#each task.dependencies ?? [] as dep}
          <div class="dep-item">← Blocks: {dep}</div>
        {/each}
      </div>
    </div>
    {/if}
  </div>
</aside>

<style>
  .task-detail { width: 480px; background: #161B22; border-left: 1px solid #30363D; height: 100%; overflow-y: auto; display: flex; flex-direction: column; }
  .detail-header { padding: 16px 20px; border-bottom: 1px solid #30363D; display: flex; flex-direction: column; gap: 8px; }
  .back-btn { background: none; border: none; color: #8B949E; cursor: pointer; font-size: 14px; align-self: flex-start; padding: 0; }
  .detail-title { margin: 0; font-size: 18px; color: #E6EDF3; font-weight: 600; }
  .title-input { font-size: 18px; font-weight: 600; background: #0D1117; border: 1px solid #FF8C00; border-radius: 4px; color: #E6EDF3; padding: 4px 8px; width: 100%; }
  .title-text { cursor: pointer; }
  .detail-body { padding: 16px 20px; display: flex; flex-direction: column; gap: 16px; }
  .field-row { display: flex; align-items: center; gap: 16px; }
  .field-label { font-size: 12px; color: #6E7681; min-width: 80px; }
  .field-select { background: #0D1117; border: 1px solid #30363D; border-radius: 6px; color: #E6EDF3; padding: 4px 8px; font-size: 14px; }
  .priority-badge { font-size: 14px; color: #FF8C00; }
  .field-section { display: flex; flex-direction: column; gap: 8px; }
  .section-header { display: flex; align-items: center; justify-content: space-between; }
  .section-label { font-size: 13px; font-weight: 600; color: #8B949E; }
  .progress-text { color: #6E7681; font-weight: 400; }
  .add-btn { background: none; border: 1px solid #30363D; border-radius: 6px; color: #8B949E; cursor: pointer; font-size: 12px; padding: 2px 8px; }
  .inline-input { background: #0D1117; border: 1px solid #FF8C00; border-radius: 6px; color: #E6EDF3; padding: 6px 10px; font-size: 13px; width: 100%; }
  .subtask-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 4px; }
  .subtask-item { display: flex; align-items: center; gap: 8px; font-size: 13px; color: #E6EDF3; }
  .description-text { font-size: 14px; color: #8B949E; line-height: 1.5; margin: 0; }
  .tags-list { display: flex; flex-wrap: wrap; gap: 4px; }
  .tag { font-size: 12px; background: #21262D; color: #8B949E; padding: 2px 8px; border-radius: 4px; }
  .time-entries { list-style: none; padding: 0; margin: 0; font-size: 13px; color: #8B949E; }
  .attachment-list { list-style: none; padding: 0; margin: 0; }
  .attachment-item { font-size: 13px; color: #58A6FF; padding: 4px 0; }
  .dep-item { font-size: 13px; color: #8B949E; }
</style>
