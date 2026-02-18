<script lang="ts">
  type SearchMode = 'keyword' | 'semantic' | 'hybrid';

  let {
    mode = 'keyword' as SearchMode,
    threshold = 0.5,
    offline = false,
    onSearch,
    onClear,
    onModeChange,
  } = $props<{
    mode?: SearchMode;
    threshold?: number;
    offline?: boolean;
    onSearch?: (params: { query: string; mode: SearchMode; threshold?: number }) => void;
    onClear?: () => void;
    onModeChange?: (mode: SearchMode) => void;
  }>();

  let query = $state('');
  let currentMode = $state<SearchMode>(mode);
  let currentThreshold = $state(threshold);
  let hasSearch = $state(false);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const modes: { value: SearchMode; label: string }[] = [
    { value: 'keyword', label: 'Keyword' },
    { value: 'semantic', label: 'Semantic' },
    { value: 'hybrid', label: 'Hybrid' },
  ];

  function handleInput() {
    if (debounceTimer) clearTimeout(debounceTimer);
    hasSearch = query.length > 0;
    if (currentMode === 'keyword') {
      debounceTimer = setTimeout(() => {
        onSearch?.({ query, mode: currentMode });
      }, 300);
    }
  }

  function handleSubmit(e: Event) {
    e.preventDefault();
    if (query.trim()) {
      onSearch?.({ query, mode: currentMode, threshold: currentMode !== 'keyword' ? currentThreshold : undefined });
    }
  }

  function handleClear() {
    query = '';
    hasSearch = false;
    if (debounceTimer) clearTimeout(debounceTimer);
    onClear?.();
  }

  function selectMode(m: SearchMode) {
    currentMode = m;
    onModeChange?.(m);
  }
</script>

<div class="search-bar" data-testid="search-bar">
  {#if offline}
    <div class="offline-msg" data-testid="offline-msg">
      Offline — keyword only
    </div>
  {/if}

  <div class="mode-selector" data-testid="mode-selector">
    {#each modes as m}
      <button
        class="mode-btn"
        class:active={currentMode === m.value}
        onclick={() => selectMode(m.value)}
        aria-pressed={currentMode === m.value}
        data-testid="mode-{m.value}"
      >
        {m.label}
      </button>
    {/each}
  </div>

  <form class="search-form" onsubmit={handleSubmit}>
    <input
      type="search"
      class="search-input"
      bind:value={query}
      oninput={handleInput}
      placeholder="Search tasks..."
      aria-label="Search tasks"
      data-testid="search-input"
    />
    {#if hasSearch}
      <button
        type="button"
        class="clear-btn"
        onclick={handleClear}
        aria-label="Clear search"
        data-testid="clear-btn"
      >
        Clear search
      </button>
    {/if}
    <button type="submit" class="search-btn" aria-label="Submit search" data-testid="search-btn">
      🔍
    </button>
  </form>

  {#if currentMode === 'semantic' || currentMode === 'hybrid'}
    <div class="threshold-row" data-testid="threshold-row">
      <label for="threshold-slider" class="threshold-label">
        Threshold: {currentThreshold.toFixed(1)}
      </label>
      <input
        id="threshold-slider"
        type="range"
        class="threshold-slider"
        bind:value={currentThreshold}
        min="0"
        max="1"
        step="0.1"
        role="slider"
        aria-valuemin={0}
        aria-valuemax={1}
        aria-valuenow={currentThreshold}
        aria-valuetext="{currentThreshold.toFixed(1)}"
        data-testid="threshold-slider"
      />
    </div>
  {/if}
</div>

<style>
  .search-bar { display: flex; flex-direction: column; gap: 8px; padding: 8px; }
  .offline-msg { font-size: 12px; color: #D29922; background: rgba(210,153,34,0.15); padding: 4px 8px; border-radius: 4px; }
  .mode-selector { display: flex; gap: 2px; }
  .mode-btn { background: #21262D; border: 1px solid #30363D; color: #8B949E; padding: 4px 10px; border-radius: 6px; cursor: pointer; font-size: 12px; }
  .mode-btn.active { background: #FF8C00; color: #0D1117; border-color: #FF8C00; }
  .search-form { display: flex; gap: 4px; align-items: center; }
  .search-input { flex: 1; background: #0D1117; border: 1px solid #30363D; border-radius: 6px; color: #E6EDF3; padding: 6px 12px; font-size: 14px; }
  .clear-btn { background: none; border: none; color: #8B949E; cursor: pointer; font-size: 12px; white-space: nowrap; }
  .search-btn { background: #21262D; border: 1px solid #30363D; color: #8B949E; padding: 6px 10px; border-radius: 6px; cursor: pointer; }
  .threshold-row { display: flex; align-items: center; gap: 8px; }
  .threshold-label { font-size: 12px; color: #8B949E; }
  .threshold-slider { flex: 1; accent-color: #FF8C00; }
</style>
