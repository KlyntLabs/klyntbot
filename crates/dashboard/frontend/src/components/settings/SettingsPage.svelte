<script lang="ts">
  import ProviderForm from './ProviderForm.svelte';
  import ThresholdSlider from './ThresholdSlider.svelte';

  export let config: any = {};
  export let onSave: (section: string, data: any) => void = () => {};

  const tabs = ['Providers', 'Channels', 'Tools', 'Notifications', 'Calendar', 'Search'];
  let activeTab = 'Providers';

  let providers = config.providers ?? [];
  let apiKeyChanged = false;

  function handleProviderUpdate(updated: any[]) {
    providers = updated;
    apiKeyChanged = true;
  }

  function saveSection() {
    if (activeTab === 'Providers') {
      onSave('providers', { providers });
    }
  }
</script>

<div class="settings-page">
  <h1>Settings</h1>

  <nav class="tabs" role="tablist" aria-label="Settings sections">
    {#each tabs as tab}
      <button
        role="tab"
        aria-selected={activeTab === tab}
        class:active={activeTab === tab}
        on:click={() => { activeTab = tab; apiKeyChanged = false; }}
      >
        {tab}
      </button>
    {/each}
  </nav>

  <div class="tab-content" role="tabpanel" aria-label={activeTab}>
    {#if activeTab === 'Providers'}
      <ProviderForm {providers} onUpdate={handleProviderUpdate} />
      {#if apiKeyChanged}
        <p class="restart-warning" role="alert">
          ⚠ Changes to API keys require restart
        </p>
      {/if}
    {:else if activeTab === 'Channels'}
      <div class="placeholder">Channel configuration coming soon.</div>
    {:else if activeTab === 'Tools'}
      <div class="placeholder">Tool configuration coming soon.</div>
    {:else if activeTab === 'Notifications'}
      <div class="placeholder">Notification settings coming soon.</div>
    {:else if activeTab === 'Calendar'}
      <div class="placeholder">Calendar settings coming soon.</div>
    {:else if activeTab === 'Search'}
      <div class="placeholder">Search settings coming soon.</div>
    {/if}

    <div class="actions">
      <button class="save-btn" on:click={saveSection}>Save Changes</button>
    </div>
  </div>
</div>

<style>
  .settings-page { padding: 24px; max-width: 800px; }

  h1 { font-size: 24px; color: #E6EDF3; margin: 0 0 24px; }

  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid #30363D;
    margin-bottom: 24px;
  }

  button[role="tab"] {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 8px 16px;
    color: #8B949E;
    cursor: pointer;
    font-size: 14px;
    margin-bottom: -1px;
  }

  button[role="tab"].active, button[role="tab"][aria-selected="true"] {
    color: #E6EDF3;
    border-bottom-color: #FF8C00;
  }

  button[role="tab"]:hover { color: #E6EDF3; }

  .tab-content { display: flex; flex-direction: column; gap: 16px; }

  .restart-warning {
    background: rgba(210, 153, 34, 0.15);
    border: 1px solid #D29922;
    border-radius: 6px;
    color: #D29922;
    padding: 10px 14px;
    font-size: 13px;
    margin: 0;
  }

  .placeholder { color: #6E7681; font-style: italic; }

  .actions { display: flex; justify-content: flex-end; padding-top: 16px; }

  .save-btn {
    background: #FF8C00;
    color: #0D1117;
    border: none;
    border-radius: 8px;
    padding: 8px 20px;
    font-weight: 600;
    cursor: pointer;
    font-size: 14px;
  }

  .save-btn:hover { background: #FF9D2E; }
</style>
