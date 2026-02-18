<script lang="ts">
  export let name: string;
  export let inputs: object = {};
  export let outputs: object = {};
  export let status: 'running' | 'completed' | 'failed' = 'running';
  export let expanded: boolean = false;

  function toggle() { expanded = !expanded; }
</script>

<div
  class="tool-call-card"
  class:running={status === 'running'}
  class:completed={status === 'completed'}
  class:failed={status === 'failed'}
  aria-label={`Tool call: ${name}, ${status}`}
>
  <button
    class="header"
    on:click={toggle}
    aria-expanded={expanded}
    aria-label={`${name} tool call - ${status}`}
  >
    <span class="tool-icon">
      {#if status === 'running'}
        <span class="spinner" aria-label="Running" />
      {:else if status === 'completed'}
        <span class="check" aria-label="Completed">✓</span>
      {:else}
        <span class="error" aria-label="Failed">✕</span>
      {/if}
    </span>
    <span class="tool-name">{name}</span>
    <span class="expand-icon">{expanded ? '▲' : '▼'}</span>
  </button>

  {#if expanded}
    <div class="details">
      <div class="section">
        <h4>Inputs</h4>
        <pre>{JSON.stringify(inputs, null, 2)}</pre>
      </div>
      {#if status !== 'running'}
        <div class="section">
          <h4>Outputs</h4>
          <pre>{JSON.stringify(outputs, null, 2)}</pre>
        </div>
      {/if}
    </div>
  {:else if status === 'completed'}
    <div class="summary">
      {typeof outputs === 'object' && outputs !== null
        ? JSON.stringify(outputs).substring(0, 80)
        : String(outputs)}
    </div>
  {/if}
</div>

<style>
  .tool-call-card {
    border: 1px solid #30363D;
    border-radius: 8px;
    margin: 4px 0;
    overflow: hidden;
    font-size: 12px;
  }

  .header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    width: 100%;
    text-align: left;
    background: #161B22;
    border: none;
    cursor: pointer;
    color: #E6EDF3;
  }

  .header:hover { background: #1C2128; }

  .tool-name { font-weight: 500; flex: 1; }
  .expand-icon { color: #6E7681; font-size: 10px; }

  .spinner {
    display: inline-block;
    width: 12px; height: 12px;
    border: 2px solid #30363D;
    border-top-color: #58A6FF;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .check { color: #3FB950; font-weight: bold; }
  .error { color: #F85149; font-weight: bold; }

  @keyframes spin { to { transform: rotate(360deg); } }

  .details { padding: 12px; background: #0D1117; }
  .section h4 { color: #8B949E; font-size: 11px; margin: 0 0 4px; text-transform: uppercase; }
  .section pre {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: #E6EDF3;
    background: #161B22;
    padding: 8px;
    border-radius: 4px;
    overflow-x: auto;
    margin: 0;
  }

  .summary { padding: 4px 12px 8px; color: #8B949E; font-size: 11px; }
</style>
