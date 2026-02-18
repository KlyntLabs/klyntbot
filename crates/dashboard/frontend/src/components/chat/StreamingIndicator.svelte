<script lang="ts">
  export let active: boolean = false;
</script>

{#if active}
  <div class="streaming-indicator" aria-label="Assistant is typing" aria-live="polite">
    <span class="dot" />
    <span class="dot" />
    <span class="dot" />
  </div>
{/if}

<style>
  .streaming-indicator {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 12px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #8B949E;
    animation: pulse 1.2s ease-in-out infinite;
  }

  .dot:nth-child(2) { animation-delay: 0.2s; }
  .dot:nth-child(3) { animation-delay: 0.4s; }

  @keyframes pulse {
    0%, 100% { opacity: 0.3; transform: scale(0.8); }
    50% { opacity: 1; transform: scale(1); }
  }
</style>
