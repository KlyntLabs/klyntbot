<script lang="ts">
  interface CalendarConflict {
    id: string;
    eventTitle: string;
    local: { title: string; time?: string };
    remote: { title: string; time?: string };
  }

  type Resolution = 'local' | 'remote' | 'merge';

  let {
    conflicts = [],
    onResolve,
  }: {
    conflicts?: CalendarConflict[];
    onResolve?: (payload: { id: string; resolution: Resolution }) => void;
  } = $props();
</script>

<div class="conflict-list">
  {#if conflicts.length === 0}
    <p class="no-conflicts">No conflicts</p>
  {:else}
    <ul class="conflict-items" role="list">
      {#each conflicts as conflict (conflict.id)}
        <li class="conflict-item" role="listitem">
          <h4 class="conflict-title">{conflict.eventTitle}</h4>
          <div class="diff">
            <div class="diff-side local">
              <span class="diff-label">Local</span>
              <span class="diff-value">{conflict.local.title}</span>
              {#if conflict.local.time}
                <span class="diff-time">{conflict.local.time}</span>
              {/if}
            </div>
            <span class="diff-vs">vs</span>
            <div class="diff-side remote">
              <span class="diff-label">Remote</span>
              <span class="diff-value">{conflict.remote.title}</span>
              {#if conflict.remote.time}
                <span class="diff-time">{conflict.remote.time}</span>
              {/if}
            </div>
          </div>
          <div class="resolve-actions" role="group" aria-label="Resolve conflict">
            <button
              type="button"
              class="resolve-btn local"
              onclick={() => onResolve?.({ id: conflict.id, resolution: 'local' })}
            >
              Keep Local
            </button>
            <button
              type="button"
              class="resolve-btn remote"
              onclick={() => onResolve?.({ id: conflict.id, resolution: 'remote' })}
            >
              Keep Remote
            </button>
            <button
              type="button"
              class="resolve-btn merge"
              onclick={() => onResolve?.({ id: conflict.id, resolution: 'merge' })}
            >
              Merge
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .conflict-list {
    padding: 16px;
  }
  .no-conflicts {
    color: #8B949E;
    font-size: 14px;
  }
  .conflict-items {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .conflict-item {
    background: rgba(248, 81, 73, 0.08);
    border: 1px solid rgba(248, 81, 73, 0.3);
    border-radius: 8px;
    padding: 16px;
  }
  .conflict-title {
    font-size: 14px;
    font-weight: 600;
    color: #E6EDF3;
    margin: 0 0 12px;
  }
  .diff {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 12px;
  }
  .diff-side {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .diff-label {
    font-size: 11px;
    font-weight: 600;
    color: #6E7681;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .diff-value {
    font-size: 13px;
    color: #E6EDF3;
  }
  .diff-time {
    font-size: 12px;
    color: #8B949E;
  }
  .diff-vs {
    color: #6E7681;
    font-size: 12px;
    padding-top: 18px;
  }
  .resolve-actions {
    display: flex;
    gap: 8px;
  }
  .resolve-btn {
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 13px;
    cursor: pointer;
    border: 1px solid;
    background: transparent;
  }
  .resolve-btn.local {
    color: #3FB950;
    border-color: #3FB950;
  }
  .resolve-btn.remote {
    color: #58A6FF;
    border-color: #58A6FF;
  }
  .resolve-btn.merge {
    color: #D29922;
    border-color: #D29922;
  }
</style>
