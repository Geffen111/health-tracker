<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate } from '$lib/formatDate';
  import { showToast } from '$lib/stores/toast.svelte';
  import { theme, setTheme } from '$lib/stores/theme.svelte';

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

  // App preferences (work defaults + activity defaults).
  const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  let workHours = $state(7.5);
  let workDays = $state<number[]>([1, 2, 3, 4, 5]);
  let activityDefaults = $state<string[]>(['Phone', 'Walking']);
  let activityTypeNames = $state<string[]>([]);
  let addDefault = $state('');

  let showImport = $state(false);
  let importPath = $state('G:\\Health\\Fatigue_Log_V6.xlsx');
  let importResult = $state('');
  let importing = $state(false);
  let lastImportInfo = $state('');

  let apiKey = $state('');
  let apiKeySaved = $state(false);
  let savingKey = $state(false);

  let csvRoot = $state('G:\\My Drive');
  let autoImport = $state(true);
  let lastSync = $state<string | null>(null);
  let syncing = $state(false);
  let syncMsg = $state('');
  let syncErr = $state(false);

  onMount(async () => {
    try {
      const count: any = await invoke('get_dashboard_summary');
      if (count?.date_count > 0) {
        lastImportInfo = `${count.date_count} days, ${count.headache_days_30d} with headaches, ${count.crash_count_30d} crashes.`;
      }
    } catch {}
    try {
      const k = await invoke<string | null>('get_api_key');
      if (k) { apiKey = k; apiKeySaved = true; }
    } catch {}
    try {
      const m = await invoke<string>('get_ai_model');
      if (m) aiModel = m;
    } catch {}
    try {
      const s: any = await invoke('get_sync_settings');
      if (s?.csv_root) csvRoot = s.csv_root;
      autoImport = s?.auto_import ?? true;
      lastSync = s?.last_sync ?? null;
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
      syncMsg = `Synced ${r.days_updated} day${r.days_updated === 1 ? '' : 's'} from ${r.files_processed} file${r.files_processed === 1 ? '' : 's'} (${r.files_skipped} unchanged). Steps ${r.steps_days}, HR ${r.hr_days}, sleep ${r.sleep_days}, energy ${r.energy_days}.`;
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
</script>

<div class="page-header">
  <div>
    <div class="page-title">Settings</div>
    <div class="page-subtitle">Sync, appearance, data &amp; one-time setup</div>
  </div>
</div>

<div class="settings-content">
  <div class="card">
    <div>
      <div class="card-heading">Watch &amp; health sync</div>
      <div class="card-subtitle">Reads the Samsung Health CSVs that Health Sync writes to Google Drive (steps, heart rate, sleep &amp; energy).</div>
    </div>
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

  <div class="card">
    <div>
      <div class="card-heading">AI assistant</div>
      <div class="card-subtitle">Powers the <a href="/ask" class="inline-link">Ask</a> page &amp; AI insights via OpenRouter.</div>
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
        <span class="key-status">Key saved · the Ask page is ready to use.</span>
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
  </div>

  <div class="card row-card">
    <div>
      <div class="card-heading">PEM calibration</div>
      <div class="card-subtitle">33 model parameters — view on the PEM Model screen.</div>
    </div>
    <a href="/pem-model" class="nav-link">
      Open PEM Model
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
    </a>
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
          <div class="path-row">
            <input id="import-path" bind:value={importPath} class="mono-input" />
          </div>
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
</div>

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:24px; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }

  .settings-content { max-width:760px; display:flex; flex-direction:column; gap:16px; }

  .card { background:var(--card);border:1px solid var(--border);border-radius:18px;padding:22px;box-shadow:var(--shadow);display:flex;flex-direction:column;gap:16px; }
  .row-card { flex-direction:row; align-items:center; justify-content:space-between; gap:16px; }
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

  .toggle-card-row { display:flex; align-items:center; justify-content:space-between; }
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
  .inline-link { color:var(--accent-fg); font-weight:700; text-decoration:none; }
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
  .path-row { display:flex; gap:10px; }

  .import-info { display:flex; align-items:center; gap:11px; background:var(--accent-soft); border:1px solid var(--border); border-radius:12px; padding:12px 14px; }
  .import-info span { font-size:12.5px; color:var(--accent-fg); }

  .import-actions { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  .import-hint { font-size:11.5px; color:var(--tm); line-height:1.5; max-width:380px; }
  .run-import-btn { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:11px 20px; font-size:13px; font-weight:700; cursor:pointer; white-space:nowrap; }
  .run-import-btn:disabled { opacity:.6; cursor:not-allowed; }

  .import-result { white-space:pre-wrap; font-family:monospace; font-size:12px; line-height:1.5; color:var(--ts); background:var(--inset); border-radius:10px; padding:12px; }
</style>