<script lang="ts">
  export interface DashboardStats {
    activeTasks: number;
    inFocus: number;
    eventsToday: number;
    agentStatus: 'Running' | 'Stopped';
    channelCount: number;
    calendarSyncStatus: string;
  }

  export interface ActivityItem {
    id: string;
    description: string;
    timestamp: Date;
  }

  export let stats: DashboardStats = {
    activeTasks: 0,
    inFocus: 0,
    eventsToday: 0,
    agentStatus: 'Running',
    channelCount: 0,
    calendarSyncStatus: 'Unknown',
  };
  export let recentActivity: ActivityItem[] = [];
  export let onNewTask: () => void = () => {};
  export let onStartFocus: () => void = () => {};
  export let onSyncCalendar: () => void = () => {};

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

<main class="dashboard-page" aria-label="Dashboard">
  <h1>Dashboard</h1>

  <section class="summary-cards" aria-label="Summary">
    <div class="stat-card">
      <div class="stat-value">{stats.activeTasks}</div>
      <div class="stat-label">Active Tasks</div>
    </div>
    <div class="stat-card">
      <div class="stat-value">{stats.inFocus}</div>
      <div class="stat-label">In Focus</div>
    </div>
    <div class="stat-card">
      <div class="stat-value">{stats.eventsToday}</div>
      <div class="stat-label">Events Today</div>
    </div>
  </section>

  <section class="recent-activity" aria-label="Recent Activity">
    <h2>Recent Activity</h2>
    {#if recentActivity.length === 0}
      <p class="no-activity">No recent activity.</p>
    {:else}
      <ul>
        {#each recentActivity as item (item.id)}
          <li class="activity-item">
            <span class="activity-description">{item.description}</span>
            <time class="activity-time">{relativeTime(item.timestamp)}</time>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="quick-actions" aria-label="Quick Actions">
    <button on:click={onNewTask} aria-label="New Task">+ New Task</button>
    <button on:click={onStartFocus} aria-label="Start Focus">▶ Start Focus</button>
    <button on:click={onSyncCalendar} aria-label="Sync Calendar">↻ Sync Calendar</button>
  </section>

  <section class="system-status" aria-label="System Status">
    <h2>System Status</h2>
    <div class="status-cards">
      <div class="status-card" class:ok={stats.agentStatus === 'Running'}>
        <div class="status-label">Agent</div>
        <div class="status-value">{stats.agentStatus}</div>
      </div>
      <div class="status-card">
        <div class="status-label">Channels</div>
        <div class="status-value">{stats.channelCount} connected</div>
      </div>
      <div class="status-card">
        <div class="status-label">Calendar Sync</div>
        <div class="status-value">{stats.calendarSyncStatus}</div>
      </div>
    </div>
  </section>
</main>

<style>
  .dashboard-page { padding: 24px; max-width: 1200px; }

  h1 { font-size: 24px; color: #E6EDF3; margin: 0 0 24px; }
  h2 { font-size: 16px; color: #E6EDF3; margin: 0 0 12px; font-weight: 600; }

  .summary-cards {
    display: flex;
    gap: 16px;
    margin-bottom: 32px;
  }

  .stat-card {
    flex: 1;
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 8px;
    padding: 20px;
    text-align: center;
  }

  .stat-value { font-size: 32px; font-weight: 700; color: #FF8C00; margin-bottom: 4px; }
  .stat-label { font-size: 13px; color: #8B949E; }

  .recent-activity { margin-bottom: 32px; }
  ul { list-style: none; padding: 0; margin: 0; }

  .activity-item {
    display: flex;
    justify-content: space-between;
    padding: 10px 0;
    border-bottom: 1px solid #21262D;
    font-size: 14px;
    color: #E6EDF3;
  }

  .activity-time { color: #6E7681; font-size: 12px; }

  .no-activity { color: #6E7681; font-style: italic; }

  .quick-actions {
    display: flex;
    gap: 12px;
    margin-bottom: 32px;
  }

  .quick-actions button {
    background: #21262D;
    color: #E6EDF3;
    border: 1px solid #30363D;
    border-radius: 8px;
    padding: 10px 16px;
    font-size: 14px;
    cursor: pointer;
    font-weight: 500;
  }

  .quick-actions button:hover { background: #30363D; border-color: #FF8C00; }

  .status-cards { display: flex; gap: 12px; }

  .status-card {
    flex: 1;
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 8px;
    padding: 16px;
  }

  .status-card.ok { border-color: rgba(63, 185, 80, 0.3); }

  .status-label { font-size: 11px; color: #6E7681; font-weight: 600; text-transform: uppercase; margin-bottom: 4px; }
  .status-value { font-size: 14px; color: #E6EDF3; font-weight: 500; }
</style>
