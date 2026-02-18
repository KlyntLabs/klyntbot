<script lang="ts">
  export interface Notification {
    id: string;
    type: string;
    title: string;
    body: string;
    read: boolean;
    timestamp: Date;
    href?: string;
  }

  export let notification: Notification;
  export let onClick: (n: Notification) => void = () => {};

  const typeIcons: Record<string, string> = {
    task: '✓',
    plan: '📋',
    calendar: '📅',
    focus: '⏱',
    overdue: '⚠',
    digest: '📊',
    default: '🔔',
  };

  $: icon = typeIcons[notification.type] ?? typeIcons.default;

  function relativeTime(date: Date): string {
    const diff = Date.now() - date.getTime();
    const minutes = Math.floor(diff / 60000);
    if (minutes < 1) return 'just now';
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
  }
</script>

<button
  class="notification-item"
  class:unread={!notification.read}
  on:click={() => onClick(notification)}
  aria-label={`${notification.title}: ${notification.body}`}
>
  <span class="icon">{icon}</span>
  <div class="content">
    <div class="title">{notification.title}</div>
    <div class="body">{notification.body}</div>
  </div>
  <time class="time">{relativeTime(notification.timestamp)}</time>
</button>

<style>
  .notification-item {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 16px;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    border-bottom: 1px solid #21262D;
    cursor: pointer;
    color: #E6EDF3;
  }

  .notification-item:hover { background: #1C2128; }

  .notification-item.unread { background: rgba(255, 140, 0, 0.05); }

  .icon { font-size: 16px; flex-shrink: 0; margin-top: 2px; }

  .content { flex: 1; min-width: 0; }

  .title { font-size: 13px; font-weight: 500; color: #E6EDF3; margin-bottom: 2px; }

  .body {
    font-size: 12px;
    color: #8B949E;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .time { font-size: 11px; color: #6E7681; white-space: nowrap; flex-shrink: 0; margin-top: 2px; }
</style>
