<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let today = $state(new Date().toISOString().split('T')[0]);
  let log = $state<any>({
    log_date: today,
    fatigue_rating: null,
    fatigue_desc: '',
    headache_rating: null,
    headache_desc: '',
    my_sleep_rating: null,
    phone_sleep_rating: null,
    other_symptoms: '',
    notes: ''
  });
  let saved = $state(false);

  onMount(async () => {
    try {
      const existing = await invoke('get_daily_log', { date: today });
      if (existing) Object.assign(log, existing);
    } catch {}
  });

  async function save() {
    log.log_date = today;
    await invoke('upsert_daily_log', { log });
    saved = true;
    setTimeout(() => saved = false, 2000);
  }
</script>

<h1>Daily Log</h1>
<p class="date">{today}</p>

<div class="form-grid">
  <div class="field">
    <label>Fatigue (0-10)</label>
    <div class="slider-row">
      <input type="range" min="0" max="10" step="0.5" bind:value={log.fatigue_rating} />
      <span class="value-badge">{log.fatigue_rating ?? '—'}</span>
    </div>
  </div>

  <div class="field">
    <label>Headache (0-10)</label>
    <div class="slider-row">
      <input type="range" min="0" max="10" step="0.5" bind:value={log.headache_rating} />
      <span class="value-badge">{log.headache_rating ?? '—'}</span>
    </div>
  </div>

  <div class="field">
    <label>My Sleep Rating (0-10)</label>
    <div class="slider-row">
      <input type="range" min="0" max="10" step="0.5" bind:value={log.my_sleep_rating} />
      <span class="value-badge">{log.my_sleep_rating ?? '—'}</span>
    </div>
  </div>

  <div class="field">
    <label>Phone Sleep Rating (0-10)</label>
    <div class="slider-row">
      <input type="range" min="0" max="10" step="0.5" bind:value={log.phone_sleep_rating} />
      <span class="value-badge">{log.phone_sleep_rating ?? '—'}</span>
    </div>
  </div>

  <div class="field full-width">
    <label>Fatigue Description</label>
    <textarea bind:value={log.fatigue_desc} rows="2" placeholder="e.g. Severe morning, mild afternoon"></textarea>
  </div>

  <div class="field full-width">
    <label>Headache Description</label>
    <textarea bind:value={log.headache_desc} rows="2" placeholder="e.g. Base of skull, resolved by midday"></textarea>
  </div>

  <div class="field full-width">
    <label>Other Symptoms</label>
    <textarea bind:value={log.other_symptoms} rows="3" placeholder="Brain fog, malaise, light sensitivity..."></textarea>
  </div>

  <div class="field full-width">
    <label for="daily-notes">Other Daily Notes</label>
    <textarea id="daily-notes" bind:value={log.notes} rows="3" placeholder="Anything else worth noting for the day (incl. work notes)"></textarea>
  </div>

  <button class="save-btn" onclick={save}>
    {saved ? '✓ Saved!' : 'Save Entry'}
  </button>
</div>

<style>
  h1 { margin-bottom: 4px; }
  .date { color: #666; margin-bottom: 24px; font-size: 14px; }
  :global(.dark) .date { color: #999; }

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
    max-width: 700px;
  }

  .field { display: flex; flex-direction: column; gap: 6px; }
  .full-width { grid-column: 1 / -1; }

  label { font-size: 13px; font-weight: 600; color: #555; }
  :global(.dark) label { color: #bbb; }

  .slider-row { display: flex; align-items: center; gap: 12px; }
  .slider-row input { flex: 1; }
  .value-badge {
    min-width: 36px;
    text-align: center;
    font-weight: 700;
    font-size: 18px;
  }

  textarea {
    width: 100%;
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-family: inherit;
    font-size: 14px;
    resize: vertical;
    background: #fff;
  }
  :global(.dark) textarea { background: #1e2a45; border-color: #333; color: #e0e0e0; }

  .save-btn {
    padding: 12px 32px;
    background: #1976d2;
    color: #fff;
    border: none;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    grid-column: 1 / -1;
    justify-self: start;
  }
  .save-btn:hover { background: #1565c0; }
</style>