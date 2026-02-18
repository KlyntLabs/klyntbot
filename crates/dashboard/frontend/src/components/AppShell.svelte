<script lang="ts">
  let { sidebarOpen = true, chatOpen = true, children } = $props<{
    sidebarOpen?: boolean;
    chatOpen?: boolean;
    children?: any;
  }>();

  let sidebarCollapsed = $state(!sidebarOpen);
  let chatCollapsed = $state(!chatOpen);

  $effect(() => { sidebarCollapsed = !sidebarOpen; });
  $effect(() => { chatCollapsed = !chatOpen; });
</script>

<div class="app-shell" data-testid="app-shell">
  <aside
    class="sidebar-panel"
    class:collapsed={sidebarCollapsed}
    class:open={!sidebarCollapsed}
    data-testid="sidebar-panel"
  >
    <slot name="sidebar" />
  </aside>

  <main class="main-content" data-testid="main-content">
    <slot />
  </main>

  {#if !chatCollapsed}
  <aside class="chat-panel" data-testid="chat-panel" style="width: 400px">
    <slot name="chat" />
  </aside>
  {/if}
</div>

<style>
  .app-shell {
    display: flex;
    height: 100vh;
    /* Subtle gradient gives the backdrop-filter something to blur */
    background: radial-gradient(ellipse at 20% 50%, #1a1f2e 0%, #0D1117 60%);
    color: #E6EDF3;
  }

  .sidebar-panel {
    flex-shrink: 0;
    overflow: hidden;
    /* Glassmorphism: semi-transparent + blur */
    background: rgba(13, 15, 20, 0.72);
    backdrop-filter: blur(12px) saturate(180%);
    -webkit-backdrop-filter: blur(12px) saturate(180%);
    border-right: 1px solid rgba(255, 255, 255, 0.07);
    transition: width 250ms ease-in-out, transform 250ms ease-in-out;
    /* Desktop default: full sidebar */
    width: 260px;
  }

  .sidebar-panel.collapsed {
    width: 64px;
  }

  .main-content {
    flex: 1;
    overflow: auto;
  }

  .chat-panel {
    flex-shrink: 0;
    background: #161B22;
    border-left: 1px solid #30363D;
  }

  /* Mobile (<768px): sidebar is a fixed overlay, hidden off-screen by default */
  @media (max-width: 767px) {
    .sidebar-panel {
      position: fixed;
      top: 0;
      bottom: 0;
      left: 0;
      width: 260px;
      transform: translateX(-100%);
      z-index: 200;
    }

    .sidebar-panel.open {
      transform: translateX(0);
    }

    /* Darken content when sidebar overlay is open */
    .sidebar-panel.open ~ .main-content::before {
      content: '';
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.5);
      z-index: 199;
      pointer-events: none;
    }
  }

  /* Tablet (768px–1023px): icons-only sidebar */
  @media (min-width: 768px) and (max-width: 1023px) {
    .sidebar-panel {
      width: 64px;
    }
  }

  /* Desktop (≥1024px): full sidebar, collapse allowed */
  @media (min-width: 1024px) {
    .sidebar-panel {
      width: 260px;
    }

    .sidebar-panel.collapsed {
      width: 64px;
    }
  }
</style>
