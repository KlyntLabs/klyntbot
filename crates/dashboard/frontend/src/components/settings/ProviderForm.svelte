<script lang="ts">
  import MaskedInput from './MaskedInput.svelte';

  export interface Provider {
    name: string;
    apiKey: string;
    model: string;
    connected: boolean;
    availableModels: string[];
  }

  export let providers: Provider[] = [];
  export let onUpdate: (providers: Provider[]) => void = () => {};

  function updateKey(i: number, key: string) {
    providers = providers.map((p, idx) => idx === i ? { ...p, apiKey: key } : p);
    onUpdate(providers);
  }

  function updateModel(i: number, model: string) {
    providers = providers.map((p, idx) => idx === i ? { ...p, model } : p);
    onUpdate(providers);
  }
</script>

<div class="provider-form">
  {#each providers as provider, i}
    <div class="provider-card">
      <div class="provider-header">
        <span class="provider-name">{provider.name}</span>
        <span class="status-badge" class:connected={provider.connected}>
          {provider.connected ? 'Connected' : 'Disconnected'}
        </span>
      </div>
      <div class="field">
        <label>API Key</label>
        <MaskedInput
          value={provider.apiKey}
          onChange={(v) => updateKey(i, v)}
        />
      </div>
      <div class="field">
        <label for={`model-${i}`}>Model</label>
        <select id={`model-${i}`} value={provider.model} on:change={(e) => updateModel(i, (e.target as HTMLSelectElement).value)}>
          {#each provider.availableModels as m}
            <option value={m}>{m}</option>
          {/each}
        </select>
      </div>
    </div>
  {/each}
</div>

<style>
  .provider-form { display: flex; flex-direction: column; gap: 16px; }

  .provider-card {
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 8px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .provider-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .provider-name { font-weight: 600; color: #E6EDF3; font-size: 15px; }

  .status-badge {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 9999px;
    background: rgba(248, 81, 73, 0.15);
    color: #F85149;
  }

  .status-badge.connected {
    background: rgba(63, 185, 80, 0.15);
    color: #3FB950;
  }

  .field { display: flex; flex-direction: column; gap: 4px; }
  label { font-size: 12px; color: #8B949E; font-weight: 500; }

  select {
    background: #0D1117;
    border: 1px solid #30363D;
    border-radius: 6px;
    color: #E6EDF3;
    padding: 6px 10px;
    font-size: 13px;
  }
</style>
