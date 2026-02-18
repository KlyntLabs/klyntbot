<script lang="ts">
  export let icon: string = '';
  export let title: string;
  export let description: string;
  export let action: { label: string; onClick: () => void } | null = null;
</script>

<div class="empty-state">
  {#if icon}
    <div class="icon" aria-hidden="true">{icon}</div>
  {/if}
  <h3 class="title">{title}</h3>
  <p class="description">{description}</p>
  {#if action}
    <button class="action-btn" on:click={action.onClick}>
      {action.label}
    </button>
  {/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px 24px;
    text-align: center;
    gap: 12px;
  }

  .icon { font-size: 48px; margin-bottom: 8px; }

  .title { font-size: 18px; font-weight: 600; color: #E6EDF3; margin: 0; }

  .description { font-size: 14px; color: #8B949E; max-width: 400px; margin: 0; line-height: 1.5; }

  .action-btn {
    background: #FF8C00;
    color: #0D1117;
    border: none;
    border-radius: 8px;
    padding: 8px 20px;
    font-weight: 600;
    cursor: pointer;
    font-size: 14px;
    margin-top: 8px;
  }

  .action-btn:hover { background: #FF9D2E; }
</style>
