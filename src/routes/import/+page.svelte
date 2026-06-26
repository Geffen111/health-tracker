<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { page } from '$app/stores';

  let filePath = $state('C:\\Users\\gavin\\OneDrive\\Health\\Copy of Fatigue_Log_V5 26 June.xlsx');
  let result = $state('');
  let importing = $state(false);

  async function runImport() {
    importing = true;
    result = '';
    try {
      const res = await invoke<string>('import_spreadsheet', { filePath });
      result = res;
    } catch (e) {
      result = '❌ Error: ' + e;
    } finally {
      importing = false;
    }
  }
</script>

<h1>Import Spreadsheet</h1>
<p class="subtitle">Import your Fatigue_Log_V6.xlsx data into the app. This is a one-time migration.</p>

<div class="import-card">
  <label for="path">Spreadsheet path:</label>
  <input id="path" type="text" bind:value={filePath} />
  <p class="hint">Current file: <code>C:\Users\gavin\OneDrive\Health\Copy of Fatigue_Log_V5 26 June.xlsx</code></p>

  <button class="import-btn" onclick={runImport} disabled={importing}>
    {importing ? 'Importing...' : '🚀 Import Spreadsheet'}
  </button>
</div>

{#if result}
  <div class="result-card">
    <pre>{result}</pre>
  </div>
{/if}

<div class="info">
  <h3>What gets imported</h3>
  <ul>
    <li><strong>Fatigue Log</strong> — daily entries: fatigue, headache, sleep, medications, BP, steps, HR, work hours, alcohol, sleep stages, notes</li>
    <li><strong>ActivityLog</strong> — per-activity entries with category and energy cost</li>
    <li><strong>Calibration</strong> — PEM model parameters (already seeded, will be updated from spreadsheet)</li>
  </ul>
  <p>Duplicates are automatically skipped (data upserted by date).</p>
</div>

<style>
  h1 { margin-bottom: 4px; }
  .subtitle { color: #666; margin-bottom: 24px; font-size: 14px; }
  :global(.dark) .subtitle { color: #999; }

  .import-card {
    background: #fff;
    border-radius: 12px;
    padding: 24px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    max-width: 600px;
    margin-bottom: 20px;
  }
  :global(.dark) .import-card { background: #1e2a45; }

  label { display: block; font-weight: 600; margin-bottom: 8px; font-size: 14px; }

  #path {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 14px;
    font-family: monospace;
    margin-bottom: 8px;
  }
  :global(.dark) #path { background: #2a3a5c; border-color: #444; color: #e0e0e0; }

  .hint { font-size: 12px; color: #888; margin-bottom: 16px; }
  .hint code { background: #f0f0f0; padding: 2px 6px; border-radius: 3px; }

  .import-btn {
    padding: 12px 32px;
    background: #2e7d32;
    color: #fff;
    border: none;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }
  .import-btn:hover { background: #1b5e20; }
  .import-btn:disabled { opacity: 0.6; cursor: not-allowed; }

  .result-card {
    background: #f5f5f5;
    border-radius: 12px;
    padding: 20px;
    max-width: 600px;
    margin-bottom: 20px;
    border-left: 4px solid #2e7d32;
  }
  :global(.dark) .result-card { background: #1a2a3a; border-left-color: #4caf50; }

  .result-card pre {
    white-space: pre-wrap;
    font-family: monospace;
    font-size: 13px;
    line-height: 1.5;
  }

  .info {
    max-width: 600px;
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 2px rgba(0,0,0,0.06);
  }
  :global(.dark) .info { background: #1e2a45; }

  .info h3 { font-size: 16px; margin-bottom: 12px; }
  .info ul { padding-left: 20px; }
  .info li { margin-bottom: 6px; font-size: 14px; line-height: 1.5; }
  .info p { margin-top: 12px; font-size: 13px; color: #888; }
</style>