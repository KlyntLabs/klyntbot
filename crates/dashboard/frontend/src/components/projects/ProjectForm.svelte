<script lang="ts">
  type ProjectColor = 'red' | 'orange' | 'yellow' | 'green' | 'blue' | 'purple' | 'gray';

  const PROJECT_COLORS: Array<{ value: ProjectColor; label: string; hex: string }> = [
    { value: 'red', label: 'Red', hex: '#F85149' },
    { value: 'orange', label: 'Orange', hex: '#FF8C00' },
    { value: 'yellow', label: 'Yellow', hex: '#D29922' },
    { value: 'green', label: 'Green', hex: '#3FB950' },
    { value: 'blue', label: 'Blue', hex: '#58A6FF' },
    { value: 'purple', label: 'Purple', hex: '#A855F7' },
    { value: 'gray', label: 'Gray', hex: '#8B949E' },
  ];

  interface ProjectData {
    id?: string;
    name: string;
    description?: string;
    color: ProjectColor;
    tags?: string[];
  }

  let {
    project,
    onSubmit,
    onCancel,
  }: {
    project?: ProjectData;
    onSubmit?: (data: ProjectData) => void;
    onCancel?: () => void;
  } = $props();

  let name = $state(project?.name ?? '');
  let description = $state(project?.description ?? '');
  let color = $state<ProjectColor>(project?.color ?? 'orange');
  let tagsInput = $state((project?.tags ?? []).join(', '));
  let nameError = $state('');

  function handleSubmit(e: Event) {
    e.preventDefault();
    nameError = '';
    if (!name.trim()) {
      nameError = 'Name is required';
      return;
    }
    const tags = tagsInput
      .split(',')
      .map(t => t.trim())
      .filter(Boolean);
    onSubmit?.({ id: project?.id, name: name.trim(), description: description.trim() || undefined, color, tags });
  }
</script>

<form class="project-form" onsubmit={handleSubmit} novalidate>
  <div class="field">
    <label for="project-name">Name <span aria-hidden="true">*</span></label>
    <input
      id="project-name"
      type="text"
      bind:value={name}
      placeholder="Project name"
      aria-required="true"
      aria-invalid={nameError ? 'true' : undefined}
      aria-describedby={nameError ? 'name-error' : undefined}
    />
    {#if nameError}
      <p id="name-error" class="error" role="alert">{nameError}</p>
    {/if}
  </div>

  <div class="field">
    <label for="project-description">Description</label>
    <textarea
      id="project-description"
      bind:value={description}
      placeholder="Project description"
      rows="3"
    ></textarea>
  </div>

  <div class="field">
    <span class="field-label">Color</span>
    <div class="color-picker" role="group" aria-label="Project color">
      {#each PROJECT_COLORS as c (c.value)}
        <label class="color-option">
          <input
            type="radio"
            name="project-color"
            value={c.value}
            bind:group={color}
          />
          <span
            class="color-swatch"
            style="background-color: {c.hex}"
            aria-label={c.label}
            title={c.label}
          ></span>
        </label>
      {/each}
    </div>
  </div>

  <div class="field">
    <label for="project-tags">Tags</label>
    <input
      id="project-tags"
      type="text"
      bind:value={tagsInput}
      placeholder="frontend, dashboard (comma-separated)"
    />
  </div>

  <div class="form-actions">
    {#if onCancel}
      <button type="button" class="cancel-btn" onclick={() => onCancel?.()}>Cancel</button>
    {/if}
    <button type="submit" class="submit-btn">
      {project?.id ? 'Save Changes' : 'Create Project'}
    </button>
  </div>
</form>

<style>
  .project-form {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 24px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  label, .field-label {
    font-size: 14px;
    font-weight: 500;
    color: #E6EDF3;
  }
  input, textarea {
    background: #0D1117;
    border: 1px solid #30363D;
    border-radius: 6px;
    color: #E6EDF3;
    font-size: 14px;
    padding: 8px 12px;
    outline: none;
  }
  input:focus, textarea:focus {
    border-color: #FF8C00;
    box-shadow: 0 0 0 3px rgba(255, 140, 0, 0.3);
  }
  .error {
    font-size: 12px;
    color: #F85149;
    margin: 0;
  }
  .color-picker {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .color-option {
    cursor: pointer;
  }
  .color-option input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }
  .color-swatch {
    display: block;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    transition: border-color 150ms ease;
  }
  .color-option input:checked + .color-swatch {
    border-color: #E6EDF3;
  }
  .form-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  .cancel-btn {
    background: transparent;
    border: 1px solid #30363D;
    color: #8B949E;
    border-radius: 6px;
    padding: 8px 16px;
    cursor: pointer;
  }
  .submit-btn {
    background: #FF8C00;
    color: #0D1117;
    border: none;
    border-radius: 6px;
    padding: 8px 16px;
    font-weight: 600;
    cursor: pointer;
  }
</style>
