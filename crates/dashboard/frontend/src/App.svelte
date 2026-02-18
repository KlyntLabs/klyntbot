<script lang="ts">
  import AppShell from './components/AppShell.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import TopBar from './components/TopBar.svelte';
  import TaskBoard from './components/tasks/TaskBoard.svelte';
  import TaskCreateModal from './components/tasks/TaskCreateModal.svelte';
  import NotificationDrawer from './components/notifications/NotificationDrawer.svelte';
  import DashboardPage from './components/global/DashboardPage.svelte';
  import EmptyState from './components/global/EmptyState.svelte';
  import ProjectGrid from './components/projects/ProjectGrid.svelte';
  import GoalListPage from './components/goals/GoalListPage.svelte';
  import CalendarView from './components/calendar/CalendarView.svelte';
  import SettingsPage from './components/settings/SettingsPage.svelte';
  import ChatPanel from './components/chat/ChatPanel.svelte';

  // Bug #3 fix — full routing state
  let currentPath = $state(window.location.pathname);

  // Bug #4 fix — sidebar collapse owned here, not inside AppShell
  let sidebarCollapsed = $state(false);
  let chatOpen = $state(true);

  // Bug #5 fix — notification drawer state
  let notificationDrawerOpen = $state(false);

  // Bug #6 fix — task create modal state
  let taskCreateModalOpen = $state(false);

  // Task board state (placeholder until GraphQL is wired)
  let tasks = $state<any[]>([]);
  let tasksLoading = $state(false);
  type ViewType = 'board' | 'list' | 'tree';
  let activeView = $state<ViewType>('board');
  let taskBadge = $state<number | undefined>(undefined);
  let unreadNotifications = $state(0);

  const isHomePage = $derived(currentPath === '/');
  const isTasksPage = $derived(currentPath.startsWith('/tasks') || isHomePage);

  // Bug #7 fix — breadcrumbs derived from current route
  const breadcrumbs = $derived.by(() => {
    if (isTasksPage) {
      return [
        { label: 'Tasks', href: '/tasks' },
        { label: activeView.charAt(0).toUpperCase() + activeView.slice(1) },
      ];
    }
    const routeLabels: Record<string, string> = {
      '/projects': 'Projects',
      '/goals': 'Goals',
      '/calendar': 'Calendar',
      '/plans': 'Plans',
      '/skills': 'Skills',
      '/channels': 'Channels',
      '/settings': 'Settings',
    };
    const label = routeLabels[currentPath];
    return label ? [{ label }] : [{ label: 'Dashboard' }];
  });

  function handleNavClick(href: string) {
    currentPath = href;
    window.history.pushState({}, '', href);
  }

  function handleDragEnd(event: { taskId: string; newStatus: string }) {
    tasks = tasks.map(t =>
      t.id === event.taskId ? { ...t, status: event.newStatus } : t
    );
  }

  function handleTaskCreated(task: any) {
    tasks = [...tasks, { ...task, id: crypto.randomUUID(), status: 'Todo' }];
    taskBadge = (taskBadge ?? 0) + 1;
  }

  // Chat state — populated by WebSocket when backend is connected
  let chatMessages = $state<any[]>([]);
  let chatStreaming = $state(false);

  function handleChatSend(text: string) {
    // Optimistically add user message; backend response will be streamed later
    chatMessages = [
      ...chatMessages,
      { id: crypto.randomUUID(), role: 'user', content: text, timestamp: new Date().toISOString() },
    ];
  }
</script>

<!-- Bug #4 fix: pass sidebarOpen={!sidebarCollapsed} so AppShell reacts to App state -->
<AppShell sidebarOpen={!sidebarCollapsed} {chatOpen}>
  {#snippet sidebar()}
    <!-- Bug #4: onToggle toggles App-level sidebarCollapsed -->
    <!-- Bug #5: onBellClick toggles notification drawer -->
    <!-- Bug #3: onNavClick updates currentPath -->
    <Sidebar
      collapsed={sidebarCollapsed}
      onToggle={() => (sidebarCollapsed = !sidebarCollapsed)}
      onBellClick={() => (notificationDrawerOpen = !notificationDrawerOpen)}
      onNavClick={handleNavClick}
      activeHref={currentPath}
      {taskBadge}
      {unreadNotifications}
    />
  {/snippet}

  {#snippet chat()}
    <ChatPanel
      messages={chatMessages}
      streaming={chatStreaming}
      onSend={handleChatSend}
    />
  {/snippet}

  <div class="page-wrapper">
    <TopBar
      {breadcrumbs}
      showViewToggle={isTasksPage}
      {activeView}
      onViewChange={(v) => (activeView = v)}
      actions={[
        {
          label: '+ New Task',
          onClick: () => (taskCreateModalOpen = true),
        },
      ]}
    />

    <!-- Bug #3 fix: full router renders correct page per route -->
    <div class="page-content">
      {#if isHomePage}
        <DashboardPage
          onNewTask={() => (taskCreateModalOpen = true)}
        />
      {:else if isTasksPage}
        <TaskBoard
          {tasks}
          loading={tasksLoading}
          onDragEnd={handleDragEnd}
        />
      {:else if currentPath === '/projects'}
        <ProjectGrid />
      {:else if currentPath === '/goals'}
        <GoalListPage />
      {:else if currentPath === '/calendar'}
        <CalendarView />
      {:else if currentPath === '/plans'}
        <EmptyState
          icon="🗺️"
          title="Plans"
          description="No plans yet. Create your first plan using the CLI."
          action={{ label: 'Learn more', onClick: () => handleNavClick('/') }}
        />
      {:else if currentPath === '/skills'}
        <EmptyState
          icon="⚡"
          title="Skills"
          description="No skills configured yet. Add skills via klyntbot skills."
        />
      {:else if currentPath === '/channels'}
        <EmptyState
          icon="📡"
          title="Channels"
          description="No channels connected. Run klyntbot channels login to get started."
        />
      {:else if currentPath === '/settings'}
        <SettingsPage />
      {:else}
        <EmptyState
          icon="🔍"
          title="Page not found"
          description="This page doesn't exist."
          action={{ label: 'Go home', onClick: () => handleNavClick('/') }}
        />
      {/if}
    </div>
  </div>
</AppShell>

<!-- Bug #5 fix: NotificationDrawer mounted, open controlled by bell click -->
<NotificationDrawer
  open={notificationDrawerOpen}
  notifications={[]}
  onMarkAllRead={() => { unreadNotifications = 0; notificationDrawerOpen = false; }}
/>

<!-- Bug #6 fix: TaskCreateModal mounted, open controlled by "+ New Task" -->
<TaskCreateModal
  open={taskCreateModalOpen}
  onClose={() => (taskCreateModalOpen = false)}
  onCreated={handleTaskCreated}
/>

<style>
  .page-wrapper {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .page-content {
    flex: 1;
    overflow: auto;
  }
</style>
