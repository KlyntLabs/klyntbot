<script lang="ts">
  type SyncStatus = 'idle' | 'syncing' | 'error' | 'never';

  let {
    status = 'idle',
    lastSync = null,
    onSync,
  }: {
    status?: SyncStatus;
    lastSync?: Date | null;
    onSync?: () => void;
  } = $props();

  function formatRelativeTime(date: Date | null): string {
    if (!date) return 'Never synced';
    const diff = Math.floor((Date.now() - date.getTime()) / 1000);
    if (diff < 60) return 'Just now';
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  }

  const lastSyncLabel = $derived(
    status === 'syncing'
      ? 'Syncing...'
      : status === 'error'
      ? 'Sync failed'
      : status === 'never'
      ? 'Never synced'
      : `Last synced: ${formatRelativeTime(lastSync)}`
  );
</script>

<div class="sync-controls">
  <div class="sync-status">
    {#if status === 'syncing'}
      <span class="spinner" aria-hidden="true"></span>
    {:else if status === 'error'}
      <span class="status-dot error" aria-hidden="true"></span>
    {:else if status === 'idle'}
      <span class="status-dot ok" aria-hidden="true"></span>
    {/if}
    <span class="status-label" aria-live="polite">{lastSyncLabel}</span>
  </div>

  <button
    type="button"
    class="sync-btn"
    disabled={status === 'syncing'}
    onclick={() => onSync?.()}
    aria-label={status === 'syncing' ? 'Syncing...' : 'Sync now'}
  >
    {#if status === 'syncing'}
      Syncing...
    {:else}
      Sync Now
    {/if}
  </button>
</div>

<style>
  .sync-controls {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 8px;
  }
  .sync-status {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
  }
  .status-label {
    font-size: 13px;
    color: #8B949E;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .status-dot.ok {
    background: #3FB950;
  }
  .status-dot.error {
    background: #F85149;
  }
  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid #30363D;
    border-top-color: #FF8C00;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  .sync-btn {
    background: #21262D;
    border: 1px solid #30363D;
    color: #E6EDF3;
    border-radius: 6px;
    padding: 6px 14px;
    font-size: 13px;
    cursor: pointer;
    transition: background 150ms ease;
    white-space: nowrap;
  }
  .sync-btn:hover:not(:disabled) {
    background: #30363D;
  }
  .sync-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
