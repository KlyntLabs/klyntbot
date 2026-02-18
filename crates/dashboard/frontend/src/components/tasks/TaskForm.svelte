<script lang="ts">
  interface Task {
    id?: string;
    title: string;
    description?: string;
    priority?: number;
    dueDate?: string;
    tags?: string[];
    projectId?: string | null;
    status?: 'Todo' | 'Doing' | 'Done';
  }

  interface FormData {
    title: string;
    description: string;
    priority: number | '';
    dueDate: string;
    tags: string[];
    projectId: string;
  }

  let {
    task,
    onSubmit,
    onCancel,
    projects = [],
  } = $props<{
    task?: Task;
    onSubmit?: (data: FormData) => void;
    onCancel?: () => void;
    projects?: { id: string; name: string }[];
  }>();

  let title = $state(task?.title ?? '');
  let description = $state(task?.description ?? '');
  let priority = $state<number | ''>(task?.priority ?? '');
  let dueDate = $state(task?.dueDate ? task.dueDate.split('T')[0] : '');
  let tagInput = $state(task?.tags?.join(', ') ?? '');
  let projectId = $state(task?.projectId ?? '');

  let errors = $state<Record<string, string>>({});
  let warnings = $state<Record<string, string>>({});

  function validateTitle() {
    if (!title.trim()) {
      errors = { ...errors, title: 'Title is required' };
    } else if (title.length > 200) {
      errors = { ...errors, title: 'Max 200 characters' };
    } else {
      const { title: _, ...rest } = errors;
      errors = rest;
    }
  }

  function validatePriority() {
    const p = Number(priority);
    if (priority !== '' && (p < 1 || p > 4 || !Number.isInteger(p))) {
      errors = { ...errors, priority: 'Priority must be 1-4' };
    } else {
      const { priority: _, ...rest } = errors;
      errors = rest;
    }
  }

  function validateDueDate() {
    if (dueDate) {
      const selected = new Date(dueDate);
      const today = new Date();
      today.setHours(0, 0, 0, 0);
      if (selected < today) {
        warnings = { ...warnings, dueDate: 'This date is in the past' };
      } else {
        const { dueDate: _, ...rest } = warnings;
        warnings = rest;
      }
    }
  }

  function handleSubmit() {
    validateTitle();
    if (Object.keys(errors).length > 0) return;
    const tags = tagInput
      .split(',')
      .map(t => t.trim())
      .filter(Boolean);
    onSubmit?.({ title, description, priority, dueDate, tags, projectId });
  }

  function handleCancel() {
    onCancel?.();
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') handleCancel();
  }
</script>

<form
  class="task-form"
  data-testid="task-form"
  onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}
  onkeydown={handleKeyDown}
>
  <div class="form-field">
    <label for="task-title" class="form-label">Title <span class="required">*</span></label>
    <input
      id="task-title"
      type="text"
      class="form-input"
      class:error={'title' in errors}
      bind:value={title}
      onblur={validateTitle}
      placeholder="Task title..."
      data-testid="title-input"
      aria-required="true"
      aria-invalid={'title' in errors}
      aria-describedby={'title' in errors ? 'title-error' : undefined}
    />
    {#if errors.title}
      <span id="title-error" class="error-msg" role="alert" data-testid="title-error">
        {errors.title}
      </span>
    {/if}
  </div>

  <div class="form-field">
    <label for="task-desc" class="form-label">Description</label>
    <textarea
      id="task-desc"
      class="form-textarea"
      bind:value={description}
      placeholder="Task description..."
      data-testid="description-input"
      rows="3"
    ></textarea>
  </div>

  <div class="form-row">
    <div class="form-field">
      <label for="task-priority" class="form-label">Priority</label>
      <select
        id="task-priority"
        class="form-select"
        class:error={'priority' in errors}
        bind:value={priority}
        onblur={validatePriority}
        data-testid="priority-select"
      >
        <option value="">Select...</option>
        <option value={1}>1 — Critical</option>
        <option value={2}>2 — High</option>
        <option value={3}>3 — Medium</option>
        <option value={4}>4 — Low</option>
      </select>
      {#if errors.priority}
        <span class="error-msg" role="alert" data-testid="priority-error">{errors.priority}</span>
      {/if}
    </div>

    <div class="form-field">
      <label for="task-due" class="form-label">Due Date</label>
      <input
        id="task-due"
        type="date"
        class="form-input"
        bind:value={dueDate}
        onblur={validateDueDate}
        data-testid="due-date-input"
      />
      {#if warnings.dueDate}
        <span class="warning-msg" data-testid="due-date-warning">{warnings.dueDate}</span>
      {/if}
    </div>
  </div>

  <div class="form-field">
    <label for="task-tags" class="form-label">Tags</label>
    <input
      id="task-tags"
      type="text"
      class="form-input"
      bind:value={tagInput}
      placeholder="tag1, tag2, tag3"
      data-testid="tags-input"
    />
  </div>

  {#if projects.length > 0}
  <div class="form-field">
    <label for="task-project" class="form-label">Project</label>
    <select
      id="task-project"
      class="form-select"
      bind:value={projectId}
      data-testid="project-select"
    >
      <option value="">No project</option>
      {#each projects as project}
        <option value={project.id}>{project.name}</option>
      {/each}
    </select>
  </div>
  {/if}

  <div class="form-actions">
    <button type="button" class="btn-secondary" onclick={handleCancel} data-testid="cancel-btn">
      Cancel
    </button>
    <button type="submit" class="btn-primary" data-testid="submit-btn">
      {task ? 'Save' : 'Create'}
    </button>
  </div>
</form>

<style>
  .task-form { display: flex; flex-direction: column; gap: 16px; padding: 20px; }
  .form-field { display: flex; flex-direction: column; gap: 4px; }
  .form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .form-label { font-size: 13px; color: #8B949E; font-weight: 500; }
  .required { color: #F85149; }
  .form-input, .form-select, .form-textarea { background: #0D1117; border: 1px solid #30363D; border-radius: 6px; color: #E6EDF3; padding: 8px 12px; font-size: 14px; }
  .form-textarea { resize: vertical; font-family: inherit; }
  .form-input.error, .form-select.error { border-color: #F85149; box-shadow: 0 0 0 3px rgba(248,81,73,0.3); }
  .error-msg { font-size: 12px; color: #F85149; }
  .warning-msg { font-size: 12px; color: #D29922; }
  .form-actions { display: flex; justify-content: flex-end; gap: 8px; padding-top: 8px; }
  .btn-primary { background: #FF8C00; color: #0D1117; border: none; padding: 8px 20px; border-radius: 6px; cursor: pointer; font-weight: 600; font-size: 14px; }
  .btn-secondary { background: none; border: 1px solid #30363D; color: #8B949E; padding: 8px 20px; border-radius: 6px; cursor: pointer; font-size: 14px; }
</style>
