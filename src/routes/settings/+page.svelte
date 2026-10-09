<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate } from '$lib/formatDate';
  import { showToast } from '$lib/stores/toast.svelte';
  import { theme, setTheme } from '$lib/stores/theme.svelte';
  import { confirmAction } from '$lib/stores/confirm.svelte';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import type { DataLocation } from '$lib/dataLocation';
  import { features, setFeatures, type AiFeature } from '$lib/stores/features.svelte';

  type Module = 'sleep' | 'activity' | 'cardio' | 'medication' | 'food' | 'work' | 'pacing';
  const MODULES: { key: Module; label: string; about: string }[] = [
    { key: 'sleep', label: 'Sleep', about: 'Hours, quality and how rested you felt' },
    { key: 'activity', label: 'Activity', about: 'What you did and how much it took out of you' },
    { key: 'pacing', label: 'Pacing', about: 'Activity load over time, beside your fatigue' },
    { key: 'cardio', label: 'Blood pressure & heart rate', about: 'Readings from a cuff or monitor' },
    { key: 'medication', label: 'Medication', about: 'Doses, schedules and changes' },
    { key: 'food', label: 'Food & drink', about: 'What you had each day, beside how you felt' },
    { key: 'work', label: 'Work', about: 'Hours worked' },
  ];
  // What each AI feature sends, so the choice is an informed one.
  const AI_FEATURES: { key: AiFeature; label: string; about: string }[] = [
    { key: 'ai_weekly', label: 'Weekly summary', about: 'A written summary of each week. Sends that week\'s figures (averages, medication, food, lab results) — not your notes.' },
    { key: 'ai_ask', label: 'Ask', about: 'Ask questions about your log in plain English. Sends your question and the log figures needed to answer it.' },
    { key: 'ai_food_photo', label: 'Meal photos', about: 'Suggests what\'s in a photo of a meal. The photo is sent and not kept.' },
    { key: 'ai_food_tags', label: 'Food tagging & tidy-up', about: 'Gives new food items a category and flags (gluten, dairy…) and suggests clean-ups. Sends item names only.' },
    { key: 'ai_records', label: 'Records: lab charts & questions', about: 'Reads lab results out of your pathology notes, and answers questions about your records. Sends those notes\' text.' },
  ];
  let showAdvanced = $state(false);

  // Suggested OpenRouter models; the field also accepts any custom model id.
  const MODEL_SUGGESTIONS = [
    'deepseek/deepseek-v4-flash',
    'deepseek/deepseek-chat',
    'anthropic/claude-haiku-4.5',
    'anthropic/claude-sonnet-4.6',
    'google/gemini-2.5-flash',
    'openai/gpt-5-mini',
  ];
  let aiModel = $state('deepseek/deepseek-v4-flash');
  let savingModel = $state(false);
  // Image-capable model for reading food photos (the text model above can't see images).
  const VISION_SUGGESTIONS = [
    'google/gemini-3.8-flash',
    'google/gemini-3.5-flash-lite',
    'openai/gpt-5.6-luna',
    'anthropic/claude-sonnet-5',
  ];
  let visionModel = $state('google/gemini-3.8-flash');
  let savingVision = $state(false);

  // App preferences (work defaults + activity defaults).
  const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  let workHours = $state(7.5);
  let workDays = $state<number[]>([1, 2, 3, 4, 5]);
  let activityDefaults = $state<string[]>(['Phone', 'Walking']);
  let activityTypeNames = $state<string[]>([]);
  let addDefault = $state('');

  let showImport = $state(false);
  let importPath = $state('');
  let importResult = $state('');
  let importing = $state(false);
  let lastImportInfo = $state('');

  let apiKey = $state('');
  let apiKeySaved = $state(false);
  let savingKey = $state(false);

  let csvRoot = $state('G:\\My Drive');
  let autoImport = $state(true);
  let lastSync = $state<string | null>(null);

  // Health Records vault (read-only Obsidian browser).
  let vaultRoot = $state('');
  let vaultStatus = $state<{ ok: boolean; text: string } | null>(null);
  let syncing = $state(false);
  let syncMsg = $state('');
  let syncErr = $state(false);

  onMount(async () => {
    try {
      const count: any = await invoke('get_dashboard_summary');
      if (count?.date_count > 0) {
        lastImportInfo = `${count.date_count} days, ${count.headache_days_30d} with headaches, ${count.bad_days_30d} at fatigue 8+.`;
      }
    } catch {}
    try {
      const k = await invoke<string | null>('get_api_key');
      if (k) { apiKey = k; apiKeySaved = true; }
    } catch {}
    try {
      const m = await invoke<string>('get_ai_model');
      if (m) aiModel = m;
      const vm = await invoke<string>('get_vision_model');
      if (vm) visionModel = vm;
    } catch {}
    try {
      const s: any = await invoke('get_sync_settings');
      if (s?.csv_root) csvRoot = s.csv_root;
      autoImport = s?.auto_import ?? true;
      lastSync = s?.last_sync ?? null;
    } catch {}
    try {
      const v: any = await invoke('get_vault_settings');
      if (v?.vault_root) vaultRoot = v.vault_root;
      checkVault();
    } catch {}
    try {
      const p: any = await invoke('get_app_prefs');
      if (p) {
        workHours = p.work_hours ?? 7.5;
        if (Array.isArray(p.work_days) && p.work_days.length) workDays = p.work_days;
        if (Array.isArray(p.activity_defaults)) activityDefaults = p.activity_defaults;
      }
      const types: any[] = await invoke('list_activity_types', { categoryId: null });
      activityTypeNames = types.map((t) => t.name);
    } catch {}
  });

  async function savePrefs() {
    try {
      await invoke('save_app_prefs', { workHours, workDays, activityDefaults });
      showToast('Preferences saved');
    } catch (e) {
      console.error('Error saving prefs:', e);
      showToast('Could not save preferences', 'error');
    }
  }

  function toggleWorkDay(i: number) {
    workDays = workDays.includes(i) ? workDays.filter((d) => d !== i) : [...workDays, i].sort();
    savePrefs();
  }

  function addActivityDefault() {
    const name = addDefault.trim();
    if (name && !activityDefaults.includes(name)) {
      activityDefaults = [...activityDefaults, name];
      savePrefs();
    }
    addDefault = '';
  }

  function removeActivityDefault(name: string) {
    activityDefaults = activityDefaults.filter((n) => n !== name);
    savePrefs();
  }

  async function saveSyncSettings() {
    try {
      await invoke('save_sync_settings', { csvRoot, autoImport });
    } catch (e) { console.error('Error saving sync settings:', e); }
  }

  async function saveVaultSettings() {
    try {
      await invoke('save_vault_settings', { vaultRoot });
      showToast('Records folder saved');
      checkVault();
    } catch (e) { console.error('Error saving vault settings:', e); }
  }
  async function chooseVault() {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const dir = await open({ directory: true, multiple: false, title: 'Choose your health records folder' });
      if (typeof dir === 'string') { vaultRoot = dir; await saveVaultSettings(); }
    } catch (e) { showToast(String(e), 'error'); }
  }
  // Say what was found, so a wrong folder shows up straight away.
  async function checkVault() {
    if (!vaultRoot.trim()) { vaultStatus = null; return; }
    try {
      const idx: any = await invoke('get_vault_index');
      if (!idx.exists) { vaultStatus = { ok: false, text: "That folder can't be found." }; return; }
      const notes: any[] = idx.notes ?? [];
      const path = notes.filter((n) => n.folder === 'Pathology Results' || n.note_type === 'pathology_result').length;
      vaultStatus = notes.length
        ? { ok: true, text: `Found ${notes.length} note${notes.length === 1 ? '' : 's'}${path ? `, ${path} of them pathology reports` : ' — no "Pathology Results" folder yet'}.` }
        : { ok: false, text: 'No .md notes found in that folder.' };
    } catch (e) { vaultStatus = { ok: false, text: String(e) }; }
  }

  async function toggleAutoImport() {
    autoImport = !autoImport;
    await saveSyncSettings();
  }

  async function syncNow(full = false) {
    syncing = true;
    syncMsg = '';
    syncErr = false;
    try {
      await saveSyncSettings();
      const r: any = await invoke('import_health_csv', { root: csvRoot, full });
      lastSync = r.last_sync;
      syncMsg = `Synced ${r.days_updated} day${r.days_updated === 1 ? '' : 's'} from ${r.files_processed} file${r.files_processed === 1 ? '' : 's'} (${r.files_skipped} unchanged). Steps ${r.steps_days}, HR ${r.hr_days}, sleep ${r.sleep_days}, energy ${r.energy_days}, ${r.bp_readings} new BP reading${r.bp_readings === 1 ? '' : 's'}.`;
      // Manual sleep entries win over the watch — say so rather than counting
      // those nights as synced.
      if (r.sleep_kept_manual > 0) {
        syncMsg += ` Kept your manual sleep entry on ${r.sleep_kept_manual} night${r.sleep_kept_manual === 1 ? '' : 's'}.`;
      }
      if (r.errors && r.errors.length) {
        syncErr = true;
        syncMsg += ` · ${r.errors.length} issue(s): ${r.errors.slice(0, 3).join('; ')}`;
        showToast(`Synced with ${r.errors.length} issue(s)`, 'info');
      } else {
        showToast(`Synced ${r.days_updated} day${r.days_updated === 1 ? '' : 's'}`);
      }
    } catch (e) {
      syncErr = true;
      syncMsg = 'Sync failed: ' + e;
      showToast('Sync failed', 'error');
    } finally {
      syncing = false;
    }
  }

  async function saveApiKey() {
    savingKey = true;
    try {
      await invoke('save_api_key', { key: apiKey.trim() });
      apiKeySaved = true;
      showToast('API key saved');
    } catch (e) {
      console.error('Error saving API key:', e);
      showToast('Could not save API key', 'error');
    } finally {
      savingKey = false;
    }
  }

  async function saveAiModel() {
    savingModel = true;
    try {
      await invoke('save_ai_model', { model: aiModel.trim() });
      showToast('AI model updated');
    } catch (e) {
      console.error('Error saving AI model:', e);
      showToast('Could not save model', 'error');
    } finally {
      savingModel = false;
    }
  }

  async function saveVisionModel() {
    savingVision = true;
    try {
      await invoke('save_vision_model', { model: visionModel.trim() });
      showToast('Photo model updated');
    } catch (e) {
      console.error('Error saving vision model:', e);
      showToast('Could not save model', 'error');
    } finally {
      savingVision = false;
    }
  }

  let exporting = $state<'' | 'csv' | 'json'>('');
  let exportMsg = $state('');
  let exportErr = $state(false);

  async function runExport(kind: 'csv' | 'json') {
    exporting = kind;
    exportMsg = '';
    exportErr = false;
    try {
      const path = await invoke<string>(kind === 'csv' ? 'export_csv' : 'export_json');
      exportMsg = `Exported to ${path}`;
      showToast(`${kind.toUpperCase()} export complete`);
    } catch (e) {
      exportErr = true;
      exportMsg = 'Export failed: ' + e;
      showToast('Export failed', 'error');
    } finally {
      exporting = '';
    }
  }

  async function chooseImportFile() {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const f = await open({ multiple: false, title: 'Choose the spreadsheet', filters: [{ name: 'Excel', extensions: ['xlsx'] }] });
      if (typeof f === 'string') importPath = f;
    } catch (e) { showToast(String(e), 'error'); }
  }

  async function runImport() {
    importing = true;
    importResult = '';
    try {
      const res = await invoke<string>('import_spreadsheet', { filePath: importPath });
      importResult = res;
    } catch (e) {
      importResult = 'Error: ' + e;
    } finally {
      importing = false;
    }
  }

  // ── Data folder (see commands/data_location.rs) ──
  let dataLoc = $state<DataLocation | null>(null);
  let moving = $state(false);
  let moveErr = $state('');
  async function loadDataLocation() {
    try { dataLoc = await invoke<DataLocation>('get_data_location'); } catch (e) { console.warn(e); }
  }
  loadDataLocation();
  async function showDataFolder() {
    if (!dataLoc) return;
    try { await revealItemInDir(dataLoc.path + (dataLoc.path.includes('/') ? '/' : '\\') + 'health.db'); } catch (e) { moveErr = String(e); }
  }
  async function moveDataFolder() {
    moveErr = '';
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const dir = await open({ directory: true, multiple: false, title: 'Choose the new data folder' });
      if (typeof dir !== 'string') return;
      const hasData = await invoke<boolean>('folder_has_data', { path: dir });
      const ok = await confirmAction(hasData
        ? { title: 'Switch to the data in that folder?', message: `${dir} already has Health Tracker data. The app will restart using it. Your current data stays where it is, untouched.`, confirmLabel: 'Switch and restart' }
        : { title: 'Move your data there?', message: `A copy of your data goes to ${dir} and the app restarts using it. The current folder is left as it is, as a backup. On your other computers, choose the new folder in Settings too.`, confirmLabel: 'Copy and restart' });
      if (!ok) return;
      moving = true;
      await invoke('change_data_location', { path: dir, mode: hasData ? 'existing' : 'copy' });
    } catch (e) {
      moveErr = String(e);
      moving = false;
    }
  }
</script>

{#snippet toggle(on: boolean, onclick: () => void, label: string)}
  <button class="toggle" class:active={on} {onclick} role="switch" aria-checked={on} aria-label={label}>
    <span class="toggle-knob"></span>
  </button>
{/snippet}

<div class="page-header">
  <div>
    <div class="page-title">Settings</div>
    <div class="page-subtitle">Your data, what you track, and optional extras</div>
  </div>
</div>

<div class="settings-content">
  <div class="card">
    <div>
      <div class="card-heading">Data folder</div>
      <div class="card-subtitle">Where this computer keeps your log. To use Health Tracker on another computer, keep this in a cloud folder (OneDrive, Dropbox, Google Drive) and choose the same folder there. Only have it open on one computer at a time.</div>
    </div>
    {#if dataLoc}
      <div class="data-path">{dataLoc.path}</div>
      <div class="field-hint">This computer: {dataLoc.machine}</div>
      {#if dataLoc.stray_copies.length}
        <div class="stray">
          <strong>Possible conflicting copies in this folder:</strong> {dataLoc.stray_copies.join(', ')}.
          A sync service makes these when two computers changed the data before syncing. The app
          doesn't use them, so anything entered only into one of those copies isn't in your log.
          Once you're sure nothing is missing, they can be deleted.
        </div>
      {/if}
    {/if}
    <div class="export-btns">
      <button class="export-btn secondary" onclick={showDataFolder} disabled={!dataLoc}>Show in folder</button>
      <button class="export-btn secondary" onclick={moveDataFolder} disabled={moving}>{moving ? 'Moving…' : 'Move to another folder…'}</button>
    </div>
    {#if moveErr}<div class="export-msg err">{moveErr}</div>{/if}
  </div>

  <div class="card">
    <div>
      <div class="card-heading">What you track</div>
      <div class="card-subtitle">Pages you don't use leave the sidebar. Nothing is deleted — switch one back on and its history is still there.</div>
    </div>
    <div class="module-grid">
      {#each MODULES as m (m.key)}
        <div class="toggle-card-row">
          <div>
            <div class="toggle-label">{m.label}</div>
            <div class="toggle-sub">{m.about}</div>
          </div>
          {@render toggle(features[m.key], () => setFeatures({ [m.key]: !features[m.key] }), m.label)}
        </div>
      {/each}
    </div>
  </div>

  <div class="card">
    <div>
      <div class="card-heading">Work defaults</div>
      <div class="card-subtitle">Pre-fills the Work page so a typical day is one click to save.</div>
    </div>
    <div class="text-field">
      <label for="work-hours">Hours for a full work day</label>
      <div class="hours-row">
        <input id="work-hours" type="number" step="0.25" min="0" bind:value={workHours} onchange={savePrefs} class="hours-input" />
        <span class="field-hint">hours</span>
      </div>
    </div>
    <div class="text-field">
      <span class="field-label">Work days</span>
      <div class="day-toggle-row">
        {#each WEEKDAYS as label, i}
          <button class="day-toggle" class:active={workDays.includes(i)} onclick={() => toggleWorkDay(i)}>{label}</button>
        {/each}
      </div>
    </div>
  </div>

  <div class="card">
    <div>
      <div class="card-heading">Activity defaults</div>
      <div class="card-subtitle">Always shown on the Activity page ready for a time. Add the ones you log most days.</div>
    </div>
    <div class="def-chips">
      {#each activityDefaults as name}
        <span class="def-chip">{name}<button class="chip-x" onclick={() => removeActivityDefault(name)} aria-label="Remove">×</button></span>
      {/each}
      {#if activityDefaults.length === 0}
        <span class="field-hint">None — add one below.</span>
      {/if}
    </div>
    <div class="key-row">
      <input list="act-type-options" bind:value={addDefault} placeholder="e.g. Walking" class="mono-input" />
      <datalist id="act-type-options">
        {#each activityTypeNames as n}<option value={n}></option>{/each}
      </datalist>
      <button class="key-save-btn" onclick={addActivityDefault} disabled={!addDefault.trim()}>Add</button>
    </div>
  </div>

  <div class="card row-card">
    <div>
      <div class="card-heading">Appearance</div>
      <div class="card-subtitle">Theme used across the app.</div>
    </div>
    <div class="theme-seg">
      <button class="theme-seg-btn" class:active={!theme.dark} onclick={() => setTheme(false)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4.5"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.4 1.4M17.6 17.6 19 19M19 5l-1.4 1.4M6.4 17.6 5 19"/></svg>
        Light
      </button>
      <button class="theme-seg-btn" class:active={theme.dark} onclick={() => setTheme(true)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/></svg>
        Dark
      </button>
    </div>
  </div>

  <div class="card">
    <div>
      <div class="card-heading">Data export</div>
      <div class="card-subtitle">Download every log for backup or analysis.</div>
    </div>
    <div class="export-btns">
      <button class="export-btn primary" onclick={() => runExport('csv')} disabled={exporting !== ''}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v11M7 11l5 4 5-4M5 20h14"/></svg>
        {exporting === 'csv' ? 'Exporting…' : 'Export CSV'}
      </button>
      <button class="export-btn secondary" onclick={() => runExport('json')} disabled={exporting !== ''}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v11M7 11l5 4 5-4M5 20h14"/></svg>
        {exporting === 'json' ? 'Exporting…' : 'Export JSON'}
      </button>
    </div>
    {#if exportMsg}
      <div class="export-msg" class:err={exportErr}>{exportMsg}</div>
    {/if}
  </div>

  {#if features.pacing}
  <div class="card row-card">
    <div>
      <div class="card-heading">Pacing &amp; activity history</div>
      <div class="card-subtitle">Activity over time, fatigue trends and the signal check.</div>
    </div>
    <a href="/pacing" class="nav-link">
      Open Pacing
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
    </a>
  </div>

  {/if}

  <button class="advanced-toggle" onclick={() => showAdvanced = !showAdvanced} aria-expanded={showAdvanced}>
    <span>
      <span class="advanced-title">Advanced</span>
      <span class="advanced-sub">Watch data import · health records folder · AI features · spreadsheet import</span>
    </span>
    <span class="collapsible-chevron" style="transform:rotate({showAdvanced ? '180deg' : '0deg'});">⌄</span>
  </button>

  {#if showAdvanced}
  <div class="card">
    <div class="toggle-card-row">
      <div>
        <div class="card-heading">Watch &amp; health sync</div>
        <div class="card-subtitle">Imports steps, heart rate, sleep, energy and blood pressure from Samsung Health via the Health Sync app, and adds watch calibration to the Cardio page.</div>
      </div>
      {@render toggle(features.health_sync, () => setFeatures({ health_sync: !features.health_sync }), 'Watch & health sync')}
    </div>
    {#if features.health_sync}
    <div class="expects-box">
      <div class="expects-title">How this works</div>
      <ol class="expects-list">
        <li>The <strong>Health Sync</strong> app (Android) exports Samsung Health data to Google Drive.</li>
        <li><strong>Google Drive for Desktop</strong> mirrors those files to a local drive on this PC — there's no cloud login here, it just reads the synced folder.</li>
        <li>Point the root below at that mirrored Drive folder. It must contain these four sub-folders (exact names):
          <span class="expects-folders">Health Sync Steps · Health Sync Heart rate · Health Sync Sleep · Health Sync Energy burned</span>
        </li>
      </ol>
      <div class="expects-note">Manually-entered values are never overwritten. A missing folder is skipped silently. Use <strong>Full re-sync</strong> the first time, then spot-check a day or two against Samsung Health.</div>
    </div>
    <div class="text-field">
      <label for="csv-path">Google Drive root folder</label>
      <input id="csv-path" bind:value={csvRoot} onchange={saveSyncSettings} class="mono-input" />
      <span class="field-hint">e.g. <code>G:\My Drive</code> — wherever Drive for Desktop mounts. Set this if your Drive uses a different letter or path.</span>
    </div>
    <div class="toggle-card-row">
      <div>
        <div class="toggle-label">Auto-import on launch</div>
        <div class="toggle-sub">{lastSync ? `Last synced ${lastSync}` : 'Not synced yet'} · steps, HR, sleep &amp; energy</div>
      </div>
      <button class="toggle" class:active={autoImport} onclick={toggleAutoImport} aria-label="Toggle auto-import">
        <span class="toggle-knob"></span>
      </button>
    </div>
    <div class="sync-actions">
      <button class="run-import-btn" onclick={() => syncNow(false)} disabled={syncing}>
        {syncing ? 'Syncing…' : 'Sync now'}
      </button>
      <button class="sync-full-btn" onclick={() => syncNow(true)} disabled={syncing}>Full re-sync</button>
    </div>
    {#if syncMsg}
      <div class="export-msg" class:err={syncErr}>{syncMsg}</div>
    {/if}
    {/if}
  </div>

  <div class="card">
    <div class="toggle-card-row">
      <div>
        <div class="card-heading">Health records folder</div>
        <div class="card-subtitle">Adds a <strong>Records</strong> page that reads a folder of notes — test results, letters, a timeline. It only ever reads them; nothing in the folder is changed.</div>
      </div>
      {@render toggle(features.vault, () => setFeatures({ vault: !features.vault }), 'Health records folder')}
    </div>
    {#if features.vault}
      <div class="text-field">
        <span class="field-label">Folder</span>
        <div class="key-row">
          <input id="vault-path" bind:value={vaultRoot} onchange={saveVaultSettings} class="mono-input" placeholder="No folder chosen" />
          <button class="key-save-btn" onclick={chooseVault}>Choose…</button>
        </div>
        {#if vaultStatus}<span class="field-hint" class:warn={!vaultStatus.ok}>{vaultStatus.text}</span>{/if}
      </div>
      <div class="expects-box">
        <div class="expects-title">How the folder needs to be set up</div>
        <ol class="expects-list">
          <li>Any folder of <strong>Markdown (.md) notes</strong> works. An <strong>Obsidian</strong> vault is ideal, but Obsidian isn't required. Notes in sub-folders are included.</li>
          <li>Each <strong>top-level folder</strong> becomes a group on the Records page, e.g. <em>Health Topics</em>, <em>Pathology Results</em>, <em>Reports</em>. A note's title is its first heading.</li>
          <li>Give each note a date: in the file name (<code>FBE_2026-03-03.md</code>) or in its frontmatter (<code>date: 2026-03-03</code>).</li>
          <li>For <strong>lab charts</strong>, keep each pathology report as its own note in a folder named exactly
            <span class="expects-folders">Pathology Results</span>
            (or add <code>type: pathology_result</code> to the frontmatter), with the results in a table: test, result, units, reference range. Notes with "index" in the name are skipped.</li>
        </ol>
        <pre class="example">---
date: 2026-03-03
type: pathology_result
---
# Full Blood Examination — 3 Mar 2026

| Test        | Result | Units | Reference |
|-------------|--------|-------|-----------|
| Haemoglobin | 155    | g/L   | 130–180   |
| Platelets   | 199    | ×10⁹/L| 150–450   |</pre>
        <div class="expects-note">Pulling numbers out of reports into charts, and asking questions about your records, use AI — switch on <strong>Records</strong> under AI features below. Browsing the notes doesn't.</div>
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="toggle-card-row">
      <div>
        <div class="card-heading">AI features</div>
        <div class="card-subtitle">Optional extras that send some of your data to an AI model through <strong>OpenRouter</strong>, using your own API key (you pay OpenRouter directly, usually cents a month). Each one is switched on separately.</div>
      </div>
      {@render toggle(features.ai, () => setFeatures({ ai: !features.ai }), 'AI features')}
    </div>
    {#if features.ai}
      <div class="expects-box">
        <div class="expects-title">Getting an API key</div>
        <ol class="expects-list">
          <li>Create an account at <strong>openrouter.ai</strong> and add a little credit (a few dollars lasts a long time).</li>
          <li>Under <strong>Keys</strong>, create a key and paste it below. It's stored only on this computer, never in your synced data folder.</li>
        </ol>
      </div>
    <div class="text-field">
      <label for="api-key">OpenRouter API key</label>
      <div class="key-row">
        <input id="api-key" type="password" bind:value={apiKey} placeholder="sk-or-..." class="mono-input" oninput={() => apiKeySaved = false} />
        <button class="key-save-btn" onclick={saveApiKey} disabled={savingKey || !apiKey.trim()}>
          {savingKey ? 'Saving…' : 'Save'}
        </button>
      </div>
      <span class="field-hint">Stored only on this device — the key is never synced to the cloud.</span>
      {#if apiKeySaved}
        <span class="key-status">Key saved.</span>
      {/if}
    </div>
    <div class="text-field">
      <label for="ai-model">Model</label>
      <div class="key-row">
        <input id="ai-model" list="model-options" bind:value={aiModel} placeholder="deepseek/deepseek-v4-flash" class="mono-input" />
        <datalist id="model-options">
          {#each MODEL_SUGGESTIONS as m}<option value={m}></option>{/each}
        </datalist>
        <button class="key-save-btn" onclick={saveAiModel} disabled={savingModel || !aiModel.trim()}>
          {savingModel ? 'Saving…' : 'Save'}
        </button>
      </div>
      <span class="field-hint">Any OpenRouter model id works. Default <code>deepseek/deepseek-v4-flash</code> is cheap &amp; fast.</span>
    </div>
    <div class="text-field">
      <label for="vision-model">Photo model</label>
      <div class="key-row">
        <input id="vision-model" list="vision-options" bind:value={visionModel} placeholder="google/gemini-3.8-flash" class="mono-input" />
        <datalist id="vision-options">
          {#each VISION_SUGGESTIONS as m}<option value={m}></option>{/each}
        </datalist>
        <button class="key-save-btn" onclick={saveVisionModel} disabled={savingVision || !visionModel.trim()}>
          {savingVision ? 'Saving…' : 'Save'}
        </button>
      </div>
      <span class="field-hint">Reads meal photos dropped on the Food &amp; Drink page — must accept images. The photo is sent to OpenRouter and not kept.</span>
    </div>
      <div class="ai-list">
        {#each AI_FEATURES as a (a.key)}
          <div class="toggle-card-row">
            <div>
              <div class="toggle-label">{a.label}{#if a.key === 'ai_records' && !features.vault}<span class="needs"> · needs a health records folder</span>{/if}</div>
              <div class="toggle-sub">{a.about}</div>
            </div>
            {@render toggle(features[a.key], () => setFeatures({ [a.key]: !features[a.key] }), a.label)}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="collapsible-card">
    <button class="collapsible-toggle" onclick={() => showImport = !showImport}>
      <div class="collapsible-left">
        <span class="collapsible-icon">
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M12 16V4M8 8l4-4 4 4M5 20h14"/></svg>
        </span>
        <div>
          <div class="card-heading">Import data</div>
          <div class="card-subtitle">One-time spreadsheet import · rarely needed after setup</div>
        </div>
      </div>
      <span class="collapsible-chevron" style="transform:rotate({showImport ? '180deg' : '0deg'});">⌄</span>
    </button>
    {#if showImport}
      <div class="collapsible-content">
        <div class="text-field">
          <label for="import-path">Fatigue Log spreadsheet (.xlsx)</label>
          <div class="key-row">
            <input id="import-path" bind:value={importPath} class="mono-input" placeholder="No file chosen" />
            <button class="key-save-btn" onclick={chooseImportFile}>Choose…</button>
          </div>
          <span class="field-hint">For moving over from the CFS/ME Fatigue Log spreadsheet this app replaced. Most people won't need it.</span>
        </div>
        {#if lastImportInfo}
          <div class="import-info">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg>
            <span>Data already imported · {lastImportInfo}</span>
          </div>
        {/if}
        <div class="import-actions">
          <span class="import-hint">Re-importing is idempotent — existing days are updated in place, not duplicated.</span>
          <button class="run-import-btn" onclick={runImport} disabled={importing}>
            {importing ? 'Importing...' : 'Run import'}
          </button>
        </div>
        {#if importResult}
          <pre class="import-result">{importResult}</pre>
        {/if}
      </div>
    {/if}
  </div>
  {/if}
</div>

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:24px; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }

  .settings-content { max-width:760px; display:flex; flex-direction:column; gap:16px; }

  .card { background:var(--card);border:1px solid var(--border);border-radius:18px;padding:22px;box-shadow:var(--shadow);display:flex;flex-direction:column;gap:16px; }
  .row-card { flex-direction:row; align-items:center; justify-content:space-between; gap:16px; }
  .data-path { font-family:ui-monospace, Consolas, monospace; font-size:12.5px; color:var(--tp); background:var(--inset); border:1px solid var(--border); border-radius:10px; padding:9px 12px; overflow-wrap:anywhere; }
  .stray { font-size:12.5px; color:var(--amber-fg); background:var(--amber-soft); border-radius:10px; padding:10px 12px; line-height:1.5; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; }

  .text-field { display:flex; flex-direction:column; gap:7px; }
  .text-field label { font-size:12px; font-weight:700; color:var(--ts); }
  .mono-input { width:100%; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:11px 13px; font-size:13px; color:var(--tp); font-family:'Public Sans',monospace; }
  .field-hint { font-size:11.5px; color:var(--tm); }
  .field-hint code { background:var(--inset); border:1px solid var(--border); border-radius:5px; padding:1px 5px; font-size:11px; }

  .expects-box { background:var(--inset); border:1px solid var(--border); border-radius:13px; padding:14px 16px; display:flex; flex-direction:column; gap:9px; }
  .expects-title { font-size:11px; letter-spacing:.06em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .expects-list { margin:0; padding-left:18px; display:flex; flex-direction:column; gap:7px; }
  .expects-list li { font-size:12.5px; color:var(--ts); line-height:1.5; }
  .expects-list strong { color:var(--tp); font-weight:700; }
  .expects-folders { display:block; margin-top:5px; font-size:11.5px; color:var(--accent-fg); background:var(--accent-soft); border:1px solid var(--border); border-radius:8px; padding:6px 9px; }
  .expects-note { font-size:11.5px; color:var(--tm); line-height:1.5; }
  .expects-note strong { color:var(--ts); font-weight:700; }
  .field-label { font-size:12px; font-weight:700; color:var(--ts); }
  .hours-row { display:flex; align-items:center; gap:10px; }
  .hours-input { width:96px; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:11px 13px; font-size:13.5px; color:var(--tp); text-align:center; font-variant-numeric:tabular-nums; }
  .day-toggle-row { display:flex; gap:6px; flex-wrap:wrap; }
  .day-toggle { width:42px; padding:9px 0; border:1px solid var(--border); background:var(--inset); color:var(--ts); border-radius:11px; font-size:12px; font-weight:700; cursor:pointer; font-family:inherit; }
  .day-toggle.active { background:var(--accent); color:#fff; border-color:var(--accent); }
  .def-chips { display:flex; gap:8px; flex-wrap:wrap; align-items:center; }
  .def-chip { display:inline-flex; align-items:center; gap:6px; font-size:12.5px; color:var(--accent-fg); background:var(--accent-soft); border:1px solid var(--border); padding:6px 12px; border-radius:999px; }
  .chip-x { border:none; background:transparent; color:var(--tm); cursor:pointer; font-size:15px; line-height:1; padding:0; }
  .sync-actions { display:flex; gap:10px; }
  .sync-full-btn { background:var(--card); color:var(--tp); border:1px solid var(--border); border-radius:999px; padding:11px 18px; font-size:13px; font-weight:700; cursor:pointer; }
  .sync-full-btn:disabled, .run-import-btn:disabled { opacity:.6; cursor:not-allowed; }

  .toggle-card-row { display:flex; align-items:center; justify-content:space-between; gap:16px; }
  .module-grid, .ai-list { display:flex; flex-direction:column; gap:12px; }
  .ai-list { border-top:1px solid var(--border); padding-top:14px; }
  .needs { font-weight:500; color:var(--amber-fg); font-size:12px; }
  .field-hint.warn { color:var(--amber-fg); }
  .example { font-family:ui-monospace, Consolas, monospace; font-size:11px; color:var(--ts); background:var(--card); border:1px solid var(--border); border-radius:9px; padding:9px 11px; overflow-x:auto; white-space:pre; margin:0; }
  .expects-list code { background:var(--card); border:1px solid var(--border); border-radius:5px; padding:0 4px; font-size:11px; }
  .advanced-toggle { display:flex; align-items:center; justify-content:space-between; gap:12px; width:100%; margin-top:10px; padding:14px 4px; background:transparent; border:none; border-top:1px solid var(--border); cursor:pointer; text-align:left; }
  .advanced-title { display:block; font-family:'Source Serif 4',serif; font-size:19px; font-weight:600; color:var(--tp); }
  .advanced-sub { display:block; font-size:12px; color:var(--tm); margin-top:2px; }
  .toggle-label { font-size:13.5px; color:var(--tp); font-weight:600; }
  .toggle-sub { font-size:11.5px; color:var(--tm); }
  .toggle { width:46px;height:26px;border-radius:999px;border:none;background:var(--border);position:relative;cursor:pointer;flex-shrink:0;padding:0;transition:background .15s; }
  .toggle.active { background:var(--accent); }
  .toggle-knob { position:absolute;top:3px;left:3px;width:20px;height:20px;border-radius:50%;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.2);transition:left .15s; }
  .toggle.active .toggle-knob { left:23px; }

  .theme-seg { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:999px; padding:3px; gap:2px; }
  .theme-seg-btn { display:inline-flex; align-items:center; gap:7px; background:transparent; border:none; border-radius:999px; padding:7px 15px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .theme-seg-btn.active { background:var(--accent); color:#fff; }

  .export-btns { display:flex; gap:10px; flex-wrap:wrap; }
  .export-btn { display:inline-flex;align-items:center;gap:8px;border:none;border-radius:999px;padding:10px 18px;font-size:13px;font-weight:700;cursor:pointer;font-family:inherit; }
  .export-btn.primary { background:var(--accent); color:#fff; }
  .export-btn.secondary { background:var(--card); color:var(--tp); border:1px solid var(--border); }
  .export-btn:disabled { opacity:.6; cursor:not-allowed; }
  .export-msg { font-size:12px; color:var(--accent-fg); background:var(--accent-soft); border:1px solid var(--border); border-radius:10px; padding:10px 12px; word-break:break-all; font-family:'Public Sans',monospace; }
  .export-msg.err { color:var(--red-fg); background:var(--red-soft); }

  .nav-link { display:inline-flex;align-items:center;gap:7px;background:var(--card);color:var(--tp);border:1px solid var(--border);border-radius:999px;padding:10px 16px;font-size:13px;font-weight:700;cursor:pointer;text-decoration:none;white-space:nowrap; }
  .key-row { display:flex; gap:10px; }
  .key-save-btn { background:var(--accent); color:#fff; border:none; border-radius:12px; padding:0 18px; font-size:13px; font-weight:700; cursor:pointer; white-space:nowrap; }
  .key-save-btn:disabled { opacity:.6; cursor:not-allowed; }
  .key-status { font-size:12px; color:var(--accent-fg); }

  .collapsible-card { background:var(--card);border:1px solid var(--border);border-radius:18px;box-shadow:var(--shadow);overflow:hidden; }
  .collapsible-toggle { width:100%; display:flex; align-items:center; justify-content:space-between; gap:12px; padding:20px 22px; background:transparent; border:none; cursor:pointer; text-align:left; }
  .collapsible-left { display:flex; align-items:center; gap:13px; }
  .collapsible-icon { width:34px;height:34px;border-radius:10px;background:var(--inset);display:flex;align-items:center;justify-content:center;color:var(--ts); }
  .collapsible-chevron { font-size:18px; color:var(--tm); transition:transform .15s; }

  .collapsible-content { padding:4px 22px 22px; border-top:1px solid var(--border); display:flex; flex-direction:column; gap:16px; }

  .import-info { display:flex; align-items:center; gap:11px; background:var(--accent-soft); border:1px solid var(--border); border-radius:12px; padding:12px 14px; }
  .import-info span { font-size:12.5px; color:var(--accent-fg); }

  .import-actions { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  .import-hint { font-size:11.5px; color:var(--tm); line-height:1.5; max-width:380px; }
  .run-import-btn { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:11px 20px; font-size:13px; font-weight:700; cursor:pointer; white-space:nowrap; }
  .run-import-btn:disabled { opacity:.6; cursor:not-allowed; }

  .import-result { white-space:pre-wrap; font-family:monospace; font-size:12px; line-height:1.5; color:var(--ts); background:var(--inset); border-radius:10px; padding:12px; }
</style>