<script lang="ts">
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
    taskCount = 0,
    progress = 0,
    onSelect,
  }: {
    project: Project;
    taskCount?: number;
    progress?: number;
    onSelect?: (project: Project) => void;
  } = $props();

  const colorMap: Record<string, string> = {
    red: '#F85149',
    orange: '#FF8C00',
    yellow: '#D29922',
    green: '#3FB950',
    blue: '#58A6FF',
    purple: '#A855F7',
    gray: '#8B949E',
  };

  function handleClick() {
    onSelect?.(project);
  }
</script>

<div
  class="project-card"
  role="button"
  tabindex="0"
  aria-label="{project.name} project"
  onclick={handleClick}
  onkeydown={(e) => e.key === 'Enter' && handleClick()}
>
  <div
    class="color-stripe"
    style="background-color: {colorMap[project.color] ?? colorMap.gray}"
    data-testid="color-stripe"
    aria-hidden="true"
  ></div>
  <div class="card-body">
    <h3 class="project-name">{project.name}</h3>
    {#if project.description}
      <p class="project-description">{project.description}</p>
    {/if}
    <div class="project-meta">
      <span class="task-count">{taskCount} tasks</span>
      <div class="progress-bar" role="progressbar" aria-valuenow={progress} aria-valuemin={0} aria-valuemax={100} aria-label="Project progress">
        <div class="progress-fill" style="width: {progress}%"></div>
      </div>
      <span class="progress-label">{progress}%</span>
    </div>
  </div>
</div>

<style>
  .project-card {
    display: flex;
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 8px;
    overflow: hidden;
    cursor: pointer;
    transition: background 250ms ease, border-color 250ms ease;
  }
  .project-card:hover {
    background: #1C2128;
    border-color: #FF8C00;
  }
  .color-stripe {
    width: 4px;
    flex-shrink: 0;
  }
  .card-body {
    padding: 16px;
    flex: 1;
  }
  .project-name {
    font-size: 16px;
    font-weight: 600;
    color: #E6EDF3;
    margin: 0 0 4px;
  }
  .project-description {
    font-size: 12px;
    color: #8B949E;
    margin: 0 0 12px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .project-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: #8B949E;
  }
  .progress-bar {
    flex: 1;
    height: 4px;
    background: #21262D;
    border-radius: 2px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: #FF8C00;
    border-radius: 2px;
    transition: width 300ms ease;
  }
</style>
