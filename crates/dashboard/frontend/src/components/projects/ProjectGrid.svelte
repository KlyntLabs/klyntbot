<script lang="ts">
  import ProjectCard from './ProjectCard.svelte';

  interface Project {
    id: string;
    name: string;
    description?: string;
    status: 'Active' | 'Paused' | 'Completed' | 'Archived';
    color: 'red' | 'orange' | 'yellow' | 'green' | 'blue' | 'purple' | 'gray';
    tags?: string[];
  }

  let {
    projects = [],
    loading = false,
    onSelect,
  }: {
    projects?: Project[];
    loading?: boolean;
    onSelect?: (project: Project) => void;
  } = $props();
</script>

{#if loading}
  <div class="project-grid" aria-label="Loading projects" aria-busy="true">
    {#each { length: 4 } as _}
      <div class="skeleton-card" aria-hidden="true">
        <div class="skeleton-stripe"></div>
        <div class="skeleton-body">
          <div class="skeleton-line"></div>
          <div class="skeleton-line short"></div>
        </div>
      </div>
    {/each}
  </div>
{:else if projects.length === 0}
  <div class="empty-state" data-testid="empty-state">
    <p>No projects yet</p>
    <button class="cta-button" type="button">+ New Project</button>
  </div>
{:else}
  <div class="project-grid" role="list" aria-label="Projects">
    {#each projects as project (project.id)}
      <div role="listitem">
        <ProjectCard {project} {onSelect} />
      </div>
    {/each}
  </div>
{/if}

<style>
  .project-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    padding: 48px;
    color: #8B949E;
    text-align: center;
  }
  .cta-button {
    background: #FF8C00;
    color: #0D1117;
    border: none;
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
  .skeleton-card {
    display: flex;
    background: #161B22;
    border: 1px solid #21262D;
    border-radius: 8px;
    overflow: hidden;
    height: 120px;
  }
  .skeleton-stripe {
    width: 4px;
    background: #21262D;
    animation: shimmer 1.5s infinite;
  }
  .skeleton-body {
    padding: 16px;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    justify-content: center;
  }
  .skeleton-line {
    height: 14px;
    background: #21262D;
    border-radius: 4px;
    animation: shimmer 1.5s infinite;
  }
  .skeleton-line.short {
    width: 60%;
  }
  @keyframes shimmer {
    0% { opacity: 0.5; }
    50% { opacity: 1; }
    100% { opacity: 0.5; }
  }
</style>
