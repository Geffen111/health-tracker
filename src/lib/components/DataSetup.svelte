<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, untrack } from 'svelte';
  import { confirmAction } from '$lib/stores/confirm.svelte';
  import type { StartupInfo, LocationOption } from '$lib/dataLocation';

  // Shown instead of the app until the database is open (see commands/data_location.rs):
  // first run (where should data live?), another computer has it open, or the folder
  // couldn't be opened.

  let { startup, onready }: { startup: StartupInfo; onready: (info: StartupInfo) => void } = $props();

  // `startup` is the state the app launched with; `current` follows what happens here.
  let current = $state<StartupInfo>(untrack(() => ({ ...startup })));
  let options = $state<LocationOption[]>([]);
  let pick = $state('');
  let busy = $state(false);
  let error = $state('');
  // From the error screen, "choose a different folder" shows the first-run choices.
  let choosing = $state(untrack(() => startup.status === 'needs_location'));

  let existing = $derived(options.filter((o) => o.has_data));
  let fresh = $derived(options.filter((o) => !o.has_data));

  onMount(async () => {
    options = await invoke<LocationOption[]>('suggest_data_locations');
    pick = fresh[0]?.path ?? '';
  });

  async function settle(p: Promise<StartupInfo>) {
    busy = true;
    error = '';
    try {
      const info = await p;
      current = info;
      if (info.status === 'ready') onready(info);
      else if (info.status === 'error') error = info.error ?? 'The folder could not be opened.';
      else choosing = false;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const choose = (path: string, mode: 'new' | 'existing') =>
    settle(invoke<StartupInfo>('choose_data_location', { path, mode }));

  async function browse(mode: 'new' | 'existing') {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const dir = await open({
        directory: true,
        multiple: false,
        title: mode === 'existing' ? 'Choose the folder your other computer uses' : 'Choose a folder for your data',
      });
      if (typeof dir === 'string') choose(dir, mode);
    } catch (e) {
      error = String(e);
    }
  }

  async function openAnyway() {
    const ok = await confirmAction({
      title: 'Open it here anyway?',
      message: `If Health Tracker really is still open on ${current.lock?.machine}, both computers will write to the same file and the sync service may make a conflicting copy. Only do this if you know it's closed there.`,
      confirmLabel: 'Open anyway',
    });
    if (ok) settle(invoke<StartupInfo>('open_despite_lock'));
  }

  function minutesAgo(unix: number): string {
    const m = Math.max(0, Math.round((Date.now() / 1000 - unix) / 60));
    return m < 1 ? 'just now' : m === 1 ? 'a minute ago' : `${m} minutes ago`;
  }
</script>

<div class="setup">
  {#if current.status === 'locked' && !choosing}
    <div class="setup-card">
      <div class="title">Health Tracker is open on {current.lock?.machine}</div>
      <p>
        It was last active there {current.lock ? minutesAgo(current.lock.heartbeat) : 'recently'}. Your data is
        kept in one file that syncs between computers, and having it open on two at once can leave
        you with conflicting copies.
      </p>
      <p>Close Health Tracker on {current.lock?.machine}, give the sync a minute to catch up, then try again.</p>
      {#if error}<div class="error">{error}</div>{/if}
      <div class="actions">
        <button class="primary" onclick={() => settle(invoke<StartupInfo>('retry_startup'))} disabled={busy}>
          {busy ? 'Checking…' : 'Try again'}
        </button>
        <button class="ghost" onclick={openAnyway} disabled={busy}>Open anyway</button>
      </div>
      <div class="hint">A computer that shut down or slept with the app open stops counting after 10 minutes.</div>
    </div>
  {:else if current.status === 'error' && !choosing}
    <div class="setup-card">
      <div class="title">Your data folder couldn't be opened</div>
      <div class="path">{current.data_dir}</div>
      <div class="error">{error || current.error}</div>
      <p>If it's on a cloud drive, check the drive is running and signed in, then try again.</p>
      <div class="actions">
        <button class="primary" onclick={() => settle(invoke<StartupInfo>('retry_startup'))} disabled={busy}>Try again</button>
        <button class="ghost" onclick={() => { choosing = true; error = ''; }}>Choose a different folder</button>
      </div>
    </div>
  {:else}
    <div class="setup-card wide">
      <div class="title">Where should your data live?</div>
      <p>
        Everything you log is kept in one file on your computer — nothing is sent anywhere. To use
        Health Tracker on more than one computer, keep it in a cloud folder (OneDrive, Dropbox,
        Google Drive) and choose the same folder on each one.
      </p>

      {#if existing.length}
        <div class="section">Found existing data</div>
        {#each existing as o (o.path)}
          <div class="option found">
            <div class="opt-text">
              <div class="opt-label">{o.label}</div>
              <div class="path">{o.path}</div>
              {#if o.modified}<div class="opt-sub">Last changed {o.modified}</div>{/if}
            </div>
            <button class="primary" onclick={() => choose(o.path, 'existing')} disabled={busy}>Use this data</button>
          </div>
        {/each}
      {/if}

      <div class="section">{existing.length ? 'Or start a new log' : 'Start a new log'}</div>
      {#each fresh as o (o.path)}
        <label class="option" class:on={pick === o.path}>
          <input type="radio" bind:group={pick} value={o.path} />
          <div class="opt-text">
            <div class="opt-label">{o.label}</div>
            <div class="opt-sub">{o.kind === 'local' ? 'Only on this computer. You can move it to a cloud folder later in Settings.' : `Syncs to your other computers through ${o.label}.`}</div>
            <div class="path">{o.path}</div>
          </div>
        </label>
      {/each}
      <div class="actions">
        <button class="primary" onclick={() => choose(pick, 'new')} disabled={busy || !pick}>{busy ? 'Opening…' : 'Start here'}</button>
        <button class="ghost" onclick={() => browse('new')} disabled={busy}>Choose another folder…</button>
      </div>

      <div class="section">Already use Health Tracker on another computer?</div>
      <p class="small">
        Choose the folder that computer uses — the one with <code>health.db</code> in it, inside your
        cloud drive. (On the other computer it's shown in Settings → Data folder.)
      </p>
      <div class="actions">
        <button class="ghost" onclick={() => browse('existing')} disabled={busy}>Choose existing folder…</button>
      </div>
      {#if error}<div class="error">{error}</div>{/if}
    </div>
  {/if}
</div>

<style>
  .setup { display:flex; justify-content:center; padding:48px 16px; }
  .setup-card { width:100%; max-width:560px; background:var(--card); border:1px solid var(--border); border-radius:20px; padding:28px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:12px; }
  .setup-card.wide { max-width:680px; }
  .title { font-family:'Source Serif 4',serif; font-size:24px; font-weight:600; color:var(--tp); }
  p { font-size:13.5px; color:var(--ts); line-height:1.6; }
  p.small { font-size:12.5px; }
  code { font-size:12px; background:var(--inset); padding:1px 5px; border-radius:5px; }
  .section { font-size:10.5px; letter-spacing:.07em; text-transform:uppercase; font-weight:800; color:var(--tm); margin-top:12px; }
  .option { display:flex; align-items:center; gap:12px; padding:12px 14px; border:1px solid var(--border); border-radius:14px; cursor:pointer; }
  .option.on { border-color:var(--accent); background:var(--accent-soft); }
  .option.found { border-color:var(--accent); cursor:default; }
  .option input { accent-color:var(--accent); }
  .opt-text { flex:1; min-width:0; display:flex; flex-direction:column; gap:2px; }
  .opt-label { font-size:14px; font-weight:700; color:var(--tp); }
  .opt-sub { font-size:12px; color:var(--ts); }
  .path { font-family:ui-monospace, Consolas, monospace; font-size:11.5px; color:var(--tm); overflow-wrap:anywhere; }
  .actions { display:flex; gap:10px; flex-wrap:wrap; margin-top:4px; }
  .primary { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:10px 18px; font-size:13px; font-weight:700; cursor:pointer; white-space:nowrap; }
  .ghost { background:var(--card); color:var(--ts); border:1px solid var(--border); border-radius:999px; padding:10px 18px; font-size:13px; font-weight:700; cursor:pointer; }
  .primary:disabled, .ghost:disabled { opacity:.6; cursor:default; }
  .error { background:var(--red-soft); color:var(--red-fg); border-radius:10px; padding:9px 12px; font-size:12.5px; overflow-wrap:anywhere; }
  .hint { font-size:11.5px; color:var(--tm); }
</style>
