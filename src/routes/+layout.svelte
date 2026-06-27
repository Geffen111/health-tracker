<script lang="ts">
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Toast from '$lib/components/Toast.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { theme, initTheme, toggleTheme } from '$lib/stores/theme.svelte';

  // Update check: this build is stamped with its git commit (vite define); CI
  // publishes a build-info.json carrying the latest commit to the rolling
  // `latest` GitHub release. The marker is fetched in Rust (latest_build_commit)
  // because the release-asset host sends no CORS header, so a webview fetch is
  // blocked. If the commits differ, surface a banner that opens the releases page.
  const GH_OWNER = 'Geffen111';
  const GH_REPO = 'health-tracker';
  let updateAvailable = $state(false);

  async function checkForUpdate() {
    // Skip in dev (no real commit stamp).
    if (typeof __APP_COMMIT__ === 'undefined' || __APP_COMMIT__ === 'dev') return;
    try {
      const latest = await invoke<string | null>('latest_build_commit', {
        owner: GH_OWNER,
        repo: GH_REPO,
      });
      if (latest && latest !== __APP_COMMIT__) updateAvailable = true;
    } catch {}
  }

  async function openLatestRelease() {
    try {
      await openUrl(`https://github.com/${GH_OWNER}/${GH_REPO}/releases/latest`);
    } catch {}
  }

  interface NavItem {
    href: string;
    label: string;
    svg: string;
  }

  const navItems: NavItem[] = [
    { href: '/', label: 'Dashboard', svg: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>' },
    { href: '/daily', label: 'Daily Log', svg: '<rect x="5" y="3" width="14" height="18" rx="2.5"/><path d="M9 8h6M9 12h6M9 16h3"/>' },
    { href: '/sleep', label: 'Sleep', svg: '<path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/>' },
    { href: '/activity', label: 'Activity', svg: '<path d="M3 12h3.5l2-6 3.5 12 2.5-6H21"/>' },
    { href: '/cardio', label: 'Cardio', svg: '<path d="M12 20C7 16 4 13 4 9.5 4 7 6 5.5 8 5.5c1.5 0 2.7.8 4 2.5 1.3-1.7 2.5-2.5 4-2.5 2 0 4 1.5 4 4C20 13 17 16 12 20Z"/>' },
    { href: '/medication', label: 'Medication', svg: '<rect x="4" y="9" width="16" height="6" rx="3"/><path d="M12 9v6"/>' },
    { href: '/work', label: 'Work', svg: '<rect x="3" y="7" width="18" height="13" rx="2.5"/><path d="M8 7V5.5A1.5 1.5 0 0 1 9.5 4h5A1.5 1.5 0 0 1 16 5.5V7"/>' },
    { href: '/pem-model', label: 'PEM Model', svg: '<path d="M5 16a7 7 0 0 1 14 0"/><path d="M12 16l3.2-3.2"/><circle cx="12" cy="16" r="1.1"/>' },
    { href: '/ask', label: 'Ask', svg: '<path d="M21 11.5a8.5 8.5 0 0 1-12.5 7.5L3 21l1.8-5A8.5 8.5 0 1 1 21 11.5Z"/><path d="M9.2 9.3a2.8 2.8 0 0 1 5.2 1.2c0 1.8-2.7 2.3-2.7 2.3"/><circle cx="12" cy="16.2" r="0.6" fill="currentColor" stroke="none"/>' },
    { href: '/settings', label: 'Settings', svg: '<path d="M4 8h16M4 16h16"/><circle cx="14" cy="8" r="2.4"/><circle cx="9" cy="16" r="2.4"/>' },
  ];

  // Collapsed (icons-only) sidebar, remembered across launches.
  let collapsed = $state(false);
  function toggleCollapse() {
    collapsed = !collapsed;
    localStorage.setItem('sidebarCollapsed', collapsed ? '1' : '0');
  }

  // Best-effort Health Sync auto-import on launch (silent; surfaced in Settings).
  onMount(async () => {
    initTheme();
    collapsed = localStorage.getItem('sidebarCollapsed') === '1';
    try {
      const s: any = await invoke('get_sync_settings');
      if (s?.auto_import) {
        await invoke('import_health_csv', { root: s.csv_root ?? null, full: false });
      }
    } catch {}
    checkForUpdate();
  });

  let { children }: { children: import('svelte').Snippet } = $props();
</script>

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link href="https://fonts.googleapis.com/css2?family=Source+Serif+4:opsz,wght@8..60,400;8..60,500;8..60,600;8..60,700&family=Public+Sans:wght@400;500;600;700&display=swap" rel="stylesheet" />
</svelte:head>

<div class="app-layout" class:dark={theme.dark}>
  <aside class="sidebar" class:collapsed>
    <div class="sidebar-brand">
      <div class="brand-icon">
        <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h3.5l2-6 3.5 12 2.5-6H21"/></svg>
      </div>
      <span class="brand-text">Health Tracker</span>
      <button class="theme-toggle" onclick={toggleTheme} aria-label="Toggle dark mode" title="Toggle dark mode">
        {#if theme.dark}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/></svg>
        {/if}
      </button>
    </div>

    <button
      class="collapse-btn"
      onclick={toggleCollapse}
      aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
      <span class="collapse-label">Collapse</span>
    </button>

    <nav class="sidebar-nav">
      {#each navItems as item}
        <a
          href={item.href}
          class="nav-item"
          class:active={$page.url.pathname === item.href}
          title={item.label}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            {@html item.svg}
          </svg>
          <span class="nav-label">{item.label}</span>
        </a>
      {/each}
    </nav>
    <div class="sidebar-footer">
      {#if updateAvailable}
        <button class="update-banner" onclick={openLatestRelease}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 16V4M7 9l5-5 5 5M5 20h14"/></svg>
          <span>Update available</span>
        </button>
      {/if}

      <div class="profile-badge">
        <span class="profile-avatar">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h3.5l2-6 3.5 12 2.5-6H21"/></svg>
        </span>
        <div class="profile-text">
          <span class="profile-name">My Health</span>
          <span class="profile-sub">Personal log</span>
        </div>
      </div>
    </div>
  </aside>
  <main class="main-content">
    {@render children()}
  </main>
</div>

<Toast />
<ConfirmDialog />

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(body) {
    font-family: 'Public Sans', sans-serif;
    overflow: hidden;
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
  }

  :global(svg) {
    display: block;
  }

  :global(input),
  :global(textarea),
  :global(button),
  :global(select) {
    font-family: inherit;
  }

  :root {
    --page: #EFF3F2; --card: #FFFFFF; --inset: #F4F9F7; --border: #E3ECE9;
    --tp: #25332F; --ts: #647873; --tm: #98A8A3;
    --accent: #4F8C81; --accent-soft: #E3EFEC; --accent-fg: #3C6E64;
    --peri: #7E8FC4; --peri-soft: #EAECF6;
    --amber: #C2974A; --amber-soft: #F2EBD8; --amber-fg: #9A7A2E;
    --red: #C0563F; --red-soft: #F4E0D9; --red-fg: #A8472F;
    --sidebar: #FBFDFC;
    --shadow: 0 4px 16px rgba(40,65,60,.05); --shadow-lg: 0 6px 22px rgba(40,65,60,.08);
  }

  :global(.dark) {
    --page: #141D1B; --card: #1D2826; --inset: #22302D; --border: #2C3835;
    --tp: #E7F0ED; --ts: #9DB0AB; --tm: #75857F;
    --accent: #62A99C; --accent-soft: rgba(98,169,156,.15); --accent-fg: #8FC9BD;
    --peri: #97A6DA; --peri-soft: rgba(151,166,218,.16);
    --amber: #D9A95C; --amber-soft: rgba(217,169,92,.16); --amber-fg: #E7C083;
    --red: #D17A62; --red-soft: rgba(209,122,98,.16); --red-fg: #E69A82;
    --sidebar: #16201E;
    --shadow: 0 4px 16px rgba(0,0,0,.28); --shadow-lg: 0 6px 22px rgba(0,0,0,.32);
  }

  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
    background: var(--page);
    color: var(--tp);
  }

  .sidebar {
    width: 236px;
    flex-shrink: 0;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    padding: 22px 16px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    transition: width 0.18s ease, min-width 0.18s ease, padding 0.18s ease;
  }
  .sidebar.collapsed {
    width: 66px;
    padding: 22px 10px;
  }

  .collapse-btn {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    margin-bottom: 8px;
    padding: 8px 12px;
    border: none;
    border-radius: 11px;
    background: transparent;
    color: var(--tm);
    font-family: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }
  .collapse-btn:hover {
    background: var(--accent-soft);
    color: var(--accent-fg);
  }
  .collapse-btn svg {
    flex-shrink: 0;
    transition: transform 0.18s ease;
  }
  .collapsed .collapse-btn {
    justify-content: center;
    padding: 8px;
  }
  .collapsed .collapse-btn svg {
    transform: rotate(180deg);
  }

  /* Collapsed (icons-only) state: hide text, centre the icons. */
  .collapsed .sidebar-brand {
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 4px 0 18px;
  }
  .collapsed .brand-text,
  .collapsed .collapse-label,
  .collapsed .nav-label,
  .collapsed .profile-text {
    display: none;
  }
  .collapsed .theme-toggle { margin-left: 0; }
  .collapsed .nav-item { justify-content: center; padding: 10px; }
  .collapsed .profile-badge { justify-content: center; padding: 11px 8px; }
  .collapsed .update-banner { justify-content: center; padding: 10px 8px; }
  .collapsed .update-banner span { display: none; }

  .sidebar-brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 4px 8px 20px;
  }

  .theme-toggle {
    margin-left: auto;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    border-radius: 50%;
    color: var(--ts);
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }
  .theme-toggle:hover {
    background: var(--accent-soft);
    color: var(--accent-fg);
  }

  .brand-icon {
    width: 34px;
    height: 34px;
    border-radius: 11px;
    background: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    flex-shrink: 0;
  }

  .brand-text {
    font-family: 'Source Serif 4', serif;
    font-size: 17px;
    font-weight: 600;
    color: var(--tp);
  }

  .sidebar-nav {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 9px 12px;
    border-radius: 11px;
    text-decoration: none;
    color: var(--ts);
    font-size: 13.5px;
    font-weight: 500;
    white-space: nowrap;
    transition: background 0.15s, color 0.15s;
  }

  .nav-item:hover {
    background: var(--accent-soft);
    color: var(--accent-fg);
  }

  .nav-item.active {
    background: var(--accent-soft);
    color: var(--accent-fg);
    font-weight: 700;
  }

  .sidebar-footer {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .update-banner {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 12px;
    border-radius: 11px;
    border: 1px solid var(--accent-soft);
    background: var(--accent-soft);
    color: var(--accent-fg);
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
    text-align: left;
    width: 100%;
  }
  .update-banner:hover { filter: brightness(0.97); }

  .profile-badge {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 12px;
    border-radius: 13px;
    background: var(--accent-soft);
  }
  .profile-avatar {
    width: 34px;
    height: 34px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .profile-avatar svg { width: 18px; height: 18px; }
  .profile-text {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
    min-width: 0;
  }
  .profile-name {
    font-size: 13px;
    font-weight: 700;
    color: var(--tp);
  }
  .profile-sub {
    font-size: 11px;
    color: var(--ts);
  }

  .main-content {
    flex: 1;
    padding: 30px 34px;
    min-width: 0;
    max-width: 1180px;
    overflow-y: auto;
  }
</style>
