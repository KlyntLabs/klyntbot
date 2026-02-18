<script lang="ts">
  export let online: boolean = true;
  export let message: string = 'Reconnecting...';
</script>

{#if !online}
  <div class="offline-banner" role="alert" aria-live="assertive">
    <span class="dot" aria-hidden="true" />
    {message}
  </div>
{/if}

<style>
  .offline-banner {
    background: #FF8C00;
    color: #0D1117;
    text-align: center;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #0D1117;
    animation: blink 1s infinite;
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }
</style>
