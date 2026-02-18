<script lang="ts">
  interface BacktrackEntry {
    id: string;
    stepIndex: number;
    stepDescription: string;
    reason: string;
    regeneratedSteps: string[];
    timestamp: string;
  }

  let {
    entries = [],
  }: {
    entries?: BacktrackEntry[];
  } = $props();

  let expanded = $state(false);

  function toggleExpanded() {
    expanded = !expanded;
  }
</script>

<div class="backtrack-timeline">
  <button
    type="button"
    class="timeline-header"
    onclick={toggleExpanded}
    aria-expanded={expanded}
    aria-controls="backtrack-entries"
  >
    <span class="timeline-title">
      Backtrack History ({entries.length} event{entries.length !== 1 ? 's' : ''})
    </span>
    <span class="toggle-icon" aria-hidden="true">{expanded ? '▲' : '▼'}</span>
  </button>

  {#if expanded}
    <div id="backtrack-entries" class="entries">
      {#if entries.length === 0}
        <p class="empty">No backtrack events</p>
      {:else}
        <ul class="timeline" role="list">
          {#each entries as entry (entry.id)}
            <li class="entry" role="listitem">
              <div class="entry-dot" aria-hidden="true"></div>
              <div class="entry-content">
                <div class="entry-header">
                  <span class="entry-step">Step {entry.stepIndex + 1}: {entry.stepDescription}</span>
                  <span class="entry-time">{new Date(entry.timestamp).toLocaleTimeString()}</span>
                </div>
                <p class="entry-reason">{entry.reason}</p>
                {#if entry.regeneratedSteps.length > 0}
                  <div class="regenerated-steps">
                    <span class="regen-label">Regenerated {entry.regeneratedSteps.length} step(s):</span>
                    <ul class="regen-list">
                      {#each entry.regeneratedSteps as step}
                        <li class="regen-step">{step}</li>
                      {/each}
                    </ul>
                  </div>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

<style>
  .backtrack-timeline {
    border: 1px solid #30363D;
    border-radius: 8px;
    overflow: hidden;
  }
  .timeline-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: #161B22;
    border: none;
    color: #E6EDF3;
    cursor: pointer;
    width: 100%;
    text-align: left;
    font-size: 14px;
    font-weight: 500;
    transition: background 150ms ease;
  }
  .timeline-header:hover {
    background: #1C2128;
  }
  .toggle-icon {
    color: #6E7681;
    font-size: 12px;
  }
  .entries {
    padding: 16px;
    background: #0D1117;
  }
  .empty {
    color: #6E7681;
    font-size: 13px;
  }
  .timeline {
    list-style: none;
    padding: 0;
    margin: 0;
    position: relative;
  }
  .timeline::before {
    content: '';
    position: absolute;
    left: 7px;
    top: 0;
    bottom: 0;
    width: 2px;
    background: #21262D;
  }
  .entry {
    display: flex;
    gap: 12px;
    padding-bottom: 16px;
    position: relative;
  }
  .entry-dot {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #D29922;
    border: 2px solid #0D1117;
    flex-shrink: 0;
    margin-top: 2px;
    z-index: 1;
  }
  .entry-content {
    flex: 1;
  }
  .entry-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 8px;
    margin-bottom: 4px;
  }
  .entry-step {
    font-size: 13px;
    font-weight: 600;
    color: #E6EDF3;
  }
  .entry-time {
    font-size: 11px;
    color: #6E7681;
    white-space: nowrap;
  }
  .entry-reason {
    font-size: 13px;
    color: #8B949E;
    margin: 0 0 8px;
  }
  .regenerated-steps {
    font-size: 12px;
  }
  .regen-label {
    color: #6E7681;
  }
  .regen-list {
    margin: 4px 0 0;
    padding-left: 16px;
    color: #8B949E;
  }
  .regen-step {
    margin-bottom: 2px;
  }
</style>
