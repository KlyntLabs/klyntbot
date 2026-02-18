<script lang="ts">
  interface BreadcrumbSegment {
    label: string;
    href?: string;
  }

  interface Action {
    label: string;
    onClick: () => void;
    variant?: 'primary' | 'secondary';
  }

  type ViewType = 'board' | 'list' | 'tree';

  let {
    breadcrumbs = [],
    actions = [],
    showViewToggle = false,
    activeView = 'board' as ViewType,
    onViewChange,
  } = $props<{
    breadcrumbs?: BreadcrumbSegment[];
    actions?: Action[];
    showViewToggle?: boolean;
    activeView?: ViewType;
    onViewChange?: (view: ViewType) => void;
  }>();

  const views: { label: string; value: ViewType }[] = [
    { label: 'Board', value: 'board' },
    { label: 'List', value: 'list' },
    { label: 'Tree', value: 'tree' },
  ];
</script>

<header class="top-bar" data-testid="top-bar">
  <nav class="breadcrumb" aria-label="Breadcrumb">
    {#each breadcrumbs as segment, i}
      {#if i > 0}<span class="breadcrumb-sep">/</span>{/if}
      {#if segment.href}
        <a href={segment.href} class="breadcrumb-link">{segment.label}</a>
      {:else}
        <span class="breadcrumb-current">{segment.label}</span>
      {/if}
    {/each}
  </nav>

  <div class="top-bar-actions">
    {#if showViewToggle}
      <div class="view-toggle" role="group" aria-label="View options" data-testid="view-toggle">
        {#each views as view}
          <button
            class="view-btn"
            class:active={activeView === view.value}
            onclick={() => onViewChange?.(view.value)}
            aria-pressed={activeView === view.value}
          >
            {view.label}
          </button>
        {/each}
      </div>
    {/if}

    {#each actions as action}
      <button
        class="action-btn"
        class:primary={action.variant === 'primary' || !action.variant}
        onclick={action.onClick}
      >
        {action.label}
      </button>
    {/each}
  </div>
</header>

<style>
  .top-bar { display: flex; align-items: center; justify-content: space-between; padding: 12px 24px; background: #161B22; border-bottom: 1px solid #30363D; gap: 16px; }
  .breadcrumb { display: flex; align-items: center; gap: 4px; }
  .breadcrumb-sep { color: #6E7681; }
  .breadcrumb-link, .breadcrumb-current { font-size: 14px; color: #8B949E; text-decoration: none; }
  .breadcrumb-current { color: #E6EDF3; }
  .top-bar-actions { display: flex; align-items: center; gap: 8px; }
  .view-toggle { display: flex; gap: 2px; }
  .view-btn { background: #21262D; border: 1px solid #30363D; color: #8B949E; padding: 4px 12px; border-radius: 6px; cursor: pointer; }
  .view-btn.active { background: #FF8C00; color: #0D1117; border-color: #FF8C00; }
  .action-btn { background: #FF8C00; color: #0D1117; border: none; padding: 6px 16px; border-radius: 6px; cursor: pointer; font-weight: 600; }
</style>
