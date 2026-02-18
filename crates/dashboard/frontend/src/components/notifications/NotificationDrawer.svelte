<script lang="ts">
  import NotificationItem from './NotificationItem.svelte';
  import type { Notification } from './NotificationItem.svelte';

  export let open: boolean = false;
  export let notifications: Notification[] = [];
  export let onMarkAllRead: () => void = () => {};
  export let onItemClick: (n: Notification) => void = () => {};

  function groupByTime(items: Notification[]) {
    const now = new Date();
    const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterdayStart = new Date(todayStart.getTime() - 86400000);

    const groups: { label: string; items: Notification[] }[] = [
      { label: 'Today', items: [] },
      { label: 'Yesterday', items: [] },
      { label: 'Earlier', items: [] },
    ];

    for (const n of items) {
      const d = new Date(n.timestamp);
      if (d >= todayStart) groups[0].items.push(n);
      else if (d >= yesterdayStart) groups[1].items.push(n);
      else groups[2].items.push(n);
    }

    return groups.filter(g => g.items.length > 0);
  }

  $: groups = groupByTime(notifications);
</script>

{#if open}
  <aside
    class="notification-drawer"
    role="dialog"
    aria-label="Notification center"
    aria-modal="true"
  >
    <div class="drawer-header">
      <h2>Notifications</h2>
      <button class="mark-all-read" on:click={onMarkAllRead}>
        Mark all read
      </button>
    </div>

    <div class="notification-list">
      {#if notifications.length === 0}
        <div class="empty">All caught up!</div>
      {:else}
        {#each groups as group}
          <div class="group">
            <div class="group-label">{group.label}</div>
            {#each group.items as n}
              <NotificationItem notification={n} onClick={onItemClick} />
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  </aside>
{/if}

<style>
  .notification-drawer {
    position: fixed;
    right: 0;
    top: 0;
    bottom: 0;
    width: 360px;
    background: #161B22;
    border-left: 1px solid #30363D;
    display: flex;
    flex-direction: column;
    z-index: 100;
    box-shadow: -8px 0 24px rgba(0,0,0,0.4);
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid #30363D;
  }

  h2 { margin: 0; font-size: 16px; color: #E6EDF3; font-weight: 600; }

  .mark-all-read {
    background: transparent;
    border: none;
    color: #58A6FF;
    cursor: pointer;
    font-size: 13px;
  }

  .mark-all-read:hover { text-decoration: underline; }

  .notification-list { flex: 1; overflow-y: auto; }

  .group-label {
    font-size: 11px;
    font-weight: 600;
    color: #6E7681;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 8px 16px 4px;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 120px;
    color: #6E7681;
    font-size: 14px;
  }
</style>
