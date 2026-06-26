<script lang="ts">
  import { page } from '$app/stores';

  const navItems = [
    { href: '/', label: 'Dashboard', icon: '📊' },
    { href: '/daily', label: 'Daily Log', icon: '📝' },
    { href: '/sleep', label: 'Sleep', icon: '😴' },
    { href: '/activity', label: 'Activity', icon: '🏃' },
    { href: '/cardio', label: 'Cardio', icon: '❤️' },
    { href: '/medication', label: 'Medication', icon: '💊' },
    { href: '/work', label: 'Work', icon: '💼' },
    { href: '/import', label: 'Import', icon: '📥' },
    { href: '/pem-model', label: 'PEM Model', icon: '🧠' },
    { href: '/settings', label: 'Settings', icon: '⚙️' },
  ];

  let sidebarCollapsed = $state(false);
  let darkMode = $state(false);

  function toggleTheme() {
    darkMode = !darkMode;
    document.documentElement.classList.toggle('dark', darkMode);
  }
</script>

<div class="app-layout" class:dark={darkMode}>
  <aside class="sidebar" class:collapsed={sidebarCollapsed}>
    <div class="sidebar-header">
      <span class="brand">{sidebarCollapsed ? 'HT' : 'Health Tracker'}</span>
      <button class="collapse-btn" onclick={() => sidebarCollapsed = !sidebarCollapsed}>
        {sidebarCollapsed ? '→' : '←'}
      </button>
    </div>
    <nav class="sidebar-nav">
      {#each navItems as item}
        <a
          href={item.href}
          class="nav-item"
          class:active={$page.url.pathname === item.href}
        >
          <span class="nav-icon">{item.icon}</span>
          {#if !sidebarCollapsed}
            <span class="nav-label">{item.label}</span>
          {/if}
        </a>
      {/each}
    </nav>
    <div class="sidebar-footer">
      <button class="theme-toggle" onclick={toggleTheme}>
        {darkMode ? '☀️' : '🌙'}
      </button>
    </div>
  </aside>
  <main class="main-content">
    <slot />
  </main>
</div>

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(body) {
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    overflow: hidden;
  }

  .app-layout {
    display: flex;
    height: 100vh;
    background: #f5f5f5;
    color: #1a1a1a;
    transition: background 0.3s, color 0.3s;
  }

  .app-layout.dark {
    background: #1a1a2e;
    color: #e0e0e0;
  }

  .sidebar {
    width: 220px;
    background: #fff;
    border-right: 1px solid #e0e0e0;
    display: flex;
    flex-direction: column;
    transition: width 0.2s, background 0.3s;
    overflow: hidden;
  }

  .dark .sidebar {
    background: #16213e;
    border-color: #2a2a4a;
  }

  .sidebar.collapsed {
    width: 60px;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    border-bottom: 1px solid #e0e0e0;
  }

  .dark .sidebar-header {
    border-color: #2a2a4a;
  }

  .brand {
    font-weight: 700;
    font-size: 18px;
    white-space: nowrap;
  }

  .collapse-btn {
    background: none;
    border: 1px solid #ddd;
    border-radius: 4px;
    padding: 2px 6px;
    cursor: pointer;
    font-size: 14px;
  }

  .sidebar-nav {
    flex: 1;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 8px;
    text-decoration: none;
    color: inherit;
    transition: background 0.15s;
    white-space: nowrap;
  }

  .nav-item:hover {
    background: rgba(0,0,0,0.05);
  }

  .dark .nav-item:hover {
    background: rgba(255,255,255,0.08);
  }

  .nav-item.active {
    background: #e3f2fd;
    font-weight: 600;
  }

  .dark .nav-item.active {
    background: #1a3a5c;
  }

  .nav-icon {
    font-size: 18px;
    width: 24px;
    text-align: center;
    flex-shrink: 0;
  }

  .nav-label {
    font-size: 14px;
  }

  .sidebar-footer {
    padding: 12px;
    border-top: 1px solid #e0e0e0;
  }

  .dark .sidebar-footer {
    border-color: #2a2a4a;
  }

  .theme-toggle {
    background: none;
    border: 1px solid #ddd;
    border-radius: 6px;
    padding: 6px 12px;
    cursor: pointer;
    font-size: 18px;
    width: 100%;
  }

  .main-content {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
  }
</style>