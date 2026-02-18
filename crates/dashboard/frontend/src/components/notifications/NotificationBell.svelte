<script lang="ts">
  export let unreadCount: number = 0;
  export let onClick: () => void = () => {};

  $: ariaLabel = unreadCount > 0
    ? `Notifications, ${unreadCount} unread`
    : 'Notifications';
</script>

<button
  class="notification-bell"
  on:click={onClick}
  aria-label={ariaLabel}
  aria-haspopup="true"
>
  <span class="bell-icon" aria-hidden="true">🔔</span>
  {#if unreadCount > 0}
    <span class="badge" aria-hidden="true">{unreadCount > 99 ? '99+' : unreadCount}</span>
  {/if}
</button>

<style>
  .notification-bell {
    position: relative;
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 8px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .notification-bell:hover { background: #21262D; }

  .notification-bell:focus-visible {
    outline: 3px solid #FF8C00;
    outline-offset: 2px;
  }

  .bell-icon { font-size: 18px; }

  .badge {
    position: absolute;
    top: 2px;
    right: 2px;
    background: #FF8C00;
    color: #0D1117;
    font-size: 10px;
    font-weight: 700;
    min-width: 16px;
    height: 16px;
    border-radius: 9999px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 3px;
    line-height: 1;
    animation: bounce 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes bounce {
    from { transform: scale(0); }
    to { transform: scale(1); }
  }
</style>
