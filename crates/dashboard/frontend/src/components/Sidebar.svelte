<script lang="ts">
  interface NavItem {
    icon: string;
    label: string;
    href: string;
    badge?: number;
    active?: boolean;
  }

  interface Session {
    id: string;
    name: string;
    lastMessage?: string;
    timestamp?: string;
    active?: boolean;
  }

  let {
    collapsed = false,
    onToggle,
    onBellClick,
    onNavClick,
    activeHref = '',
    taskBadge,
    projectBadge,
    sessions = [],
    unreadNotifications = 0,
  } = $props<{
    collapsed?: boolean;
    onToggle?: () => void;
    onBellClick?: () => void;
    onNavClick?: (href: string) => void;
    activeHref?: string;
    taskBadge?: number;
    projectBadge?: number;
    sessions?: Session[];
    unreadNotifications?: number;
  }>();

  // Lucide-style SVG paths (stroke, viewBox 0 0 24 24)
  const icons: Record<string, string> = {
    home: 'M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z M9 22V12h6v10',
    'check-square': 'M9 11l3 3L22 4 M21 12v7a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2h11',
    folder: 'M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z',
    target: 'M12 2a10 10 0 100 20A10 10 0 0012 2z M12 6a6 6 0 100 12A6 6 0 0012 6z M12 10a2 2 0 100 4 2 2 0 000-4z',
    calendar: 'M8 2v4 M16 2v4 M3 10h18 M5 4h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V6a2 2 0 012-2z',
    map: 'M1 6v16l7-4 8 4 7-4V2l-7 4-8-4-7 4z M8 2v16 M16 6v16',
    zap: 'M13 2L3 14h9l-1 8 10-12h-9l1-8z',
    radio: 'M12 12m-2 0a2 2 0 104 0 2 2 0 10-4 0 M16.24 7.76a6 6 0 010 8.49 M7.76 16.24a6 6 0 010-8.49 M19.07 4.93a10 10 0 010 14.14 M4.93 19.07a10 10 0 010-14.14',
    bell: 'M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9 M13.73 21a2 2 0 01-3.46 0',
    settings: 'M12 9a3 3 0 100 6 3 3 0 000-6z M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z',
    'chevron-left': 'M15 18l-6-6 6-6',
    'chevron-right': 'M9 18l6-6-6-6',
  };

  const navItems: NavItem[] = [
    { icon: 'home', label: 'Dashboard', href: '/' },
    { icon: 'check-square', label: 'Tasks', href: '/tasks' },
    { icon: 'folder', label: 'Projects', href: '/projects' },
    { icon: 'target', label: 'Goals', href: '/goals' },
    { icon: 'calendar', label: 'Calendar', href: '/calendar' },
    { icon: 'map', label: 'Plans', href: '/plans' },
    { icon: 'zap', label: 'Skills', href: '/skills' },
    { icon: 'radio', label: 'Channels', href: '/channels' },
  ];

  function getBadge(item: NavItem): number | undefined {
    if (item.href === '/tasks') return taskBadge;
    if (item.href === '/projects') return projectBadge;
    return item.badge;
  }
</script>

<nav
  role="navigation"
  aria-label="Main navigation"
  class="sidebar"
  class:collapsed
>
  <!-- macOS-style window controls -->
  <div class="traffic-lights" aria-hidden="true">
    <span class="dot dot-red"></span>
    <span class="dot dot-yellow"></span>
    <span class="dot dot-green"></span>
  </div>

  <div class="sidebar-header">
    {#if !collapsed}
      <span class="app-logo">Klyntbot</span>
    {/if}
    <button class="collapse-btn" onclick={onToggle} aria-label="Toggle sidebar">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="16" height="16">
        <path d={collapsed ? icons['chevron-right'] : icons['chevron-left']} />
      </svg>
    </button>
  </div>

  <div class="nav-items">
    {#each navItems as item}
      {@const badge = getBadge(item)}
      {@const active = activeHref === item.href}
      <a
        href={item.href}
        class="nav-item"
        class:active
        aria-current={active ? 'page' : undefined}
        aria-label={badge != null ? `${item.label}, ${badge} items` : item.label}
        onclick={(e) => { e.preventDefault(); onNavClick?.(item.href); }}
      >
        <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d={icons[item.icon]} />
        </svg>
        {#if !collapsed}
          <span class="nav-label">{item.label}</span>
          {#if badge != null}
            <span class="badge" data-testid="badge-{item.label.toLowerCase()}">{badge}</span>
          {/if}
        {/if}
      </a>
    {/each}
  </div>

  {#if sessions.length > 0 && !collapsed}
    <div class="session-list" data-testid="session-list">
      <div class="session-list-header">
        <span>Threads</span>
      </div>
      {#each sessions as session}
        <div
          class="session-item"
          class:active={session.active}
          data-testid="session-item"
        >
          <div class="session-avatar" aria-hidden="true">
            {session.name.charAt(0).toUpperCase()}
          </div>
          <div class="session-body">
            <span class="session-name">{session.name}</span>
            {#if session.lastMessage}
              <span class="session-preview">{session.lastMessage}</span>
            {:else}
              <span class="session-preview muted">No threads</span>
            {/if}
          </div>
          {#if session.timestamp}
            <span class="session-time">{session.timestamp}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <div class="sidebar-footer">
    <button
      class="icon-btn"
      aria-label={`Notifications${unreadNotifications > 0 ? `, ${unreadNotifications} unread` : ''}`}
      aria-haspopup="true"
      onclick={onBellClick}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="18" height="18">
        <path d={icons.bell} />
      </svg>
      {#if unreadNotifications > 0}
        <span class="badge badge-dot" data-testid="notification-badge">{unreadNotifications}</span>
      {/if}
    </button>

    <a
      href="/settings"
      class="icon-btn"
      aria-label="Settings"
      onclick={(e) => { e.preventDefault(); onNavClick?.('/settings'); }}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="18" height="18">
        <path d={icons.settings} />
      </svg>
      {#if !collapsed}
        <span class="nav-label">Settings</span>
      {/if}
    </a>
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 0;
    /* Transparent so AppShell's glassmorphism backdrop-filter shows through */
    background: transparent;
    overflow: hidden;
  }

  /* macOS traffic lights */
  .traffic-lights {
    display: flex;
    gap: 6px;
    padding: 14px 16px 6px;
  }

  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot-red    { background: #FF5F57; }
  .dot-yellow { background: #FEBC2E; }
  .dot-green  { background: #28C840; }

  .sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px 8px;
    min-height: 36px;
  }

  .app-logo {
    font-size: 13px;
    font-weight: 600;
    color: #FF8C00;
    letter-spacing: -0.01em;
  }

  .collapse-btn {
    background: none;
    border: none;
    color: #4D5562;
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    transition: color 150ms;
  }

  .collapse-btn:hover { color: #8B949E; }

  /* Nav items */
  .nav-items {
    flex: 1;
    padding: 4px 8px;
    overflow-y: auto;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 8px;
    border-radius: 6px;
    text-decoration: none;
    color: #6B7280;
    font-size: 13px;
    transition: background 120ms, color 120ms;
    white-space: nowrap;
  }

  .nav-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #C9D1D9;
  }

  .nav-item.active {
    background: rgba(255, 255, 255, 0.07);
    color: #E6EDF3;
  }

  .nav-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .nav-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    background: rgba(255, 140, 0, 0.85);
    color: #0D1117;
    border-radius: 9999px;
    font-size: 10px;
    font-weight: 600;
    padding: 1px 5px;
    line-height: 1.4;
  }

  /* Thread / session list */
  .session-list {
    padding: 4px 8px 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .session-list-header {
    font-size: 10px;
    font-weight: 600;
    color: #4D5562;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 8px 8px 4px;
  }

  .session-item {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    cursor: pointer;
    transition: background 120ms;
  }

  .session-item:hover { background: rgba(255, 255, 255, 0.04); }

  .session-item.active { background: rgba(255, 255, 255, 0.07); }

  .session-avatar {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    color: #8B949E;
    font-size: 10px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-top: 1px;
  }

  .session-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .session-name {
    font-size: 12px;
    color: #C9D1D9;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-preview {
    font-size: 11px;
    color: #4D5562;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-preview.muted { font-style: italic; }

  .session-time {
    font-size: 10px;
    color: #4D5562;
    flex-shrink: 0;
    margin-top: 2px;
  }

  /* Footer */
  .sidebar-footer {
    display: flex;
    flex-direction: column;
    padding: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    gap: 2px;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 8px;
    border-radius: 6px;
    background: none;
    border: none;
    color: #6B7280;
    font-size: 13px;
    cursor: pointer;
    text-decoration: none;
    position: relative;
    transition: background 120ms, color 120ms;
    white-space: nowrap;
  }

  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #C9D1D9;
  }

  .badge-dot {
    position: absolute;
    top: 3px;
    right: 3px;
    min-width: 16px;
    padding: 1px 4px;
  }
</style>
