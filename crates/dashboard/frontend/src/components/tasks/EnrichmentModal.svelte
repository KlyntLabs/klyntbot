<script lang="ts">
  interface Suggestion {
    field: string;
    value: string | number;
    confidence: number;
    accepted?: boolean;
    rejected?: boolean;
  }

  let {
    suggestions = [],
    onApply,
    onDismiss,
  } = $props<{
    suggestions?: Suggestion[];
    onApply?: (accepted: Suggestion[]) => void;
    onDismiss?: () => void;
  }>();

  let items = $state<(Suggestion & { accepted: boolean; rejected: boolean })[]>(
    suggestions.map(s => ({ ...s, accepted: false, rejected: false }))
  );

  function accept(index: number) {
    items = items.map((item, i) =>
      i === index ? { ...item, accepted: true, rejected: false } : item
    );
  }

  function reject(index: number) {
    items = items.map((item, i) =>
      i === index ? { ...item, accepted: false, rejected: true } : item
    );
  }

  function applySelected() {
    const accepted = items.filter(item => item.accepted);
    onApply?.(accepted);
  }
</script>

<div
  class="enrichment-modal"
  role="dialog"
  aria-modal="true"
  aria-label="AI enrichment suggestions"
  data-testid="enrichment-modal"
>
  <div class="modal-header">
    <h3 class="modal-title">AI Suggestions</h3>
    <button class="close-btn" onclick={onDismiss} aria-label="Dismiss suggestions" data-testid="dismiss-btn">✕</button>
  </div>

  <div class="suggestions-list" data-testid="suggestions-list">
    {#each items as item, i}
      <div
        class="suggestion-row"
        class:accepted={item.accepted}
        class:rejected={item.rejected}
        data-testid="suggestion-{item.field}"
      >
        <div class="suggestion-info">
          <span class="suggestion-field">{item.field}</span>
          <span class="suggestion-value">{item.value}</span>
        </div>

        <div class="confidence-bar-wrapper">
          <div
            class="confidence-bar"
            style="width: {item.confidence * 100}%"
            aria-label="{Math.round(item.confidence * 100)}% confidence"
            data-testid="confidence-bar-{item.field}"
          ></div>
          <span class="confidence-label">{Math.round(item.confidence * 100)}%</span>
        </div>

        <div class="suggestion-actions">
          <button
            class="accept-btn"
            class:active={item.accepted}
            onclick={() => accept(i)}
            data-testid="accept-{item.field}"
          >
            Accept
          </button>
          <button
            class="reject-btn"
            class:active={item.rejected}
            onclick={() => reject(i)}
            data-testid="reject-{item.field}"
          >
            Reject
          </button>
        </div>
      </div>
    {/each}
  </div>

  <div class="modal-footer">
    <button class="btn-secondary" onclick={onDismiss} data-testid="cancel-btn">
      Cancel
    </button>
    <button class="btn-primary" onclick={applySelected} data-testid="apply-btn">
      Apply Selected
    </button>
  </div>
</div>

<style>
  .enrichment-modal { background: #161B22; border: 1px solid #30363D; border-radius: 12px; width: 480px; box-shadow: 0 16px 48px rgba(0,0,0,0.5); }
  .modal-header { display: flex; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid #30363D; }
  .modal-title { font-size: 16px; color: #E6EDF3; font-weight: 600; margin: 0; }
  .close-btn { background: none; border: none; color: #8B949E; cursor: pointer; font-size: 16px; }
  .suggestions-list { padding: 12px 20px; display: flex; flex-direction: column; gap: 12px; }
  .suggestion-row { display: flex; flex-direction: column; gap: 8px; padding: 12px; background: #1C2128; border-radius: 8px; border: 1px solid #30363D; transition: border-color 150ms; }
  .suggestion-row.accepted { border-color: #3FB950; }
  .suggestion-row.rejected { border-color: #F85149; opacity: 0.6; }
  .suggestion-info { display: flex; justify-content: space-between; }
  .suggestion-field { font-size: 13px; color: #8B949E; font-weight: 500; text-transform: capitalize; }
  .suggestion-value { font-size: 14px; color: #E6EDF3; font-weight: 600; }
  .confidence-bar-wrapper { display: flex; align-items: center; gap: 8px; }
  .confidence-bar { height: 4px; background: #FF8C00; border-radius: 9999px; transition: width 300ms ease; }
  .confidence-label { font-size: 11px; color: #6E7681; }
  .suggestion-actions { display: flex; gap: 8px; }
  .accept-btn { background: rgba(63,185,80,0.1); border: 1px solid #3FB950; color: #3FB950; padding: 4px 12px; border-radius: 6px; cursor: pointer; font-size: 13px; }
  .accept-btn.active { background: #3FB950; color: #0D1117; }
  .reject-btn { background: rgba(248,81,73,0.1); border: 1px solid #F85149; color: #F85149; padding: 4px 12px; border-radius: 6px; cursor: pointer; font-size: 13px; }
  .reject-btn.active { background: #F85149; color: #0D1117; }
  .modal-footer { display: flex; justify-content: flex-end; gap: 8px; padding: 16px 20px; border-top: 1px solid #30363D; }
  .btn-primary { background: #FF8C00; color: #0D1117; border: none; padding: 8px 20px; border-radius: 6px; cursor: pointer; font-weight: 600; font-size: 14px; }
  .btn-secondary { background: none; border: 1px solid #30363D; color: #8B949E; padding: 8px 20px; border-radius: 6px; cursor: pointer; font-size: 14px; }
</style>
