<script lang="ts">
  interface Metric {
    name: string;
    current: number;
    target: number;
    unit?: string;
  }

  interface Goal {
    id: string;
    title: string;
    description?: string;
    status: 'Active' | 'Paused' | 'Achieved' | 'Abandoned';
    priority?: number;
    targetDate?: string;
    metrics?: Metric[];
    linkedProjectIds?: string[];
  }

  let {
    goals = [],
    loading = false,
    onSelect,
    onNew,
  }: {
    goals?: Goal[];
    loading?: boolean;
    onSelect?: (goal: Goal) => void;
    onNew?: () => void;
  } = $props();

  function getProgress(goal: Goal): number {
    if (!goal.metrics || goal.metrics.length === 0) return 0;
    const total = goal.metrics.reduce((sum, m) => {
      const pct = m.target > 0 ? Math.min(100, (m.current / m.target) * 100) : 0;
      return sum + pct;
    }, 0);
    return Math.round(total / goal.metrics.length);
  }
</script>

<div class="goal-list-page">
  <div class="page-header">
    <h1>Goals</h1>
    <button type="button" class="new-goal-btn" onclick={() => onNew?.()}>
      + New Goal
    </button>
  </div>

  {#if loading}
    <div class="goal-grid" aria-busy="true">
      {#each { length: 3 } as _}
        <div class="skeleton-card" aria-hidden="true"></div>
      {/each}
    </div>
  {:else if goals.length === 0}
    <div class="empty-state" data-testid="empty-state">
      <p>No goals yet. Start by creating your first goal!</p>
      <button type="button" class="new-goal-btn" onclick={() => onNew?.()}>+ New Goal</button>
    </div>
  {:else}
    <div class="goal-grid" role="list">
      {#each goals as goal (goal.id)}
        {@const progress = getProgress(goal)}
        <button
          class="goal-card"
          role="listitem"
          type="button"
          onclick={() => onSelect?.(goal)}
          aria-label="{goal.title}, {progress}% progress"
        >
          <h3 class="goal-title">{goal.title}</h3>
          {#if goal.description}
            <p class="goal-description">{goal.description}</p>
          {/if}
          <div class="progress-section">
            <div
              class="progress-bar"
              role="progressbar"
              aria-valuenow={progress}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label="{goal.title} progress"
            >
              <div class="progress-fill" style="width: {progress}%"></div>
            </div>
            <span class="progress-label">{progress}%</span>
          </div>
          {#if goal.linkedProjectIds && goal.linkedProjectIds.length > 0}
            <span class="linked-plans">{goal.linkedProjectIds.length} linked plan(s)</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .goal-list-page {
    padding: 24px;
  }
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;
  }
  h1 {
    font-size: 24px;
    font-weight: 700;
    color: #E6EDF3;
    margin: 0;
  }
  .new-goal-btn {
    background: #FF8C00;
    color: #0D1117;
    border: none;
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: background 150ms ease;
  }
  .new-goal-btn:hover {
    background: #FF9D2E;
  }
  .goal-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }
  .goal-card {
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 8px;
    padding: 20px;
    cursor: pointer;
    transition: background 250ms ease, border-color 250ms ease;
  }
  .goal-card:hover {
    background: #1C2128;
    border-color: #FF8C00;
  }
  .goal-title {
    font-size: 16px;
    font-weight: 600;
    color: #E6EDF3;
    margin: 0 0 8px;
  }
  .goal-description {
    font-size: 13px;
    color: #8B949E;
    margin: 0 0 12px;
  }
  .progress-section {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .progress-bar {
    flex: 1;
    height: 6px;
    background: #21262D;
    border-radius: 3px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: #FF8C00;
    border-radius: 3px;
    transition: width 300ms ease;
  }
  .progress-label {
    font-size: 12px;
    color: #8B949E;
    min-width: 32px;
    text-align: right;
  }
  .linked-plans {
    display: block;
    font-size: 12px;
    color: #58A6FF;
    margin-top: 8px;
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    padding: 64px;
    color: #8B949E;
    text-align: center;
  }
  .skeleton-card {
    height: 140px;
    background: #161B22;
    border: 1px solid #21262D;
    border-radius: 8px;
    animation: shimmer 1.5s infinite;
  }
  @keyframes shimmer {
    0% { opacity: 0.5; }
    50% { opacity: 1; }
    100% { opacity: 0.5; }
  }
</style>
