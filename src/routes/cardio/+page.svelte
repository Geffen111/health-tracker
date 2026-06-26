<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  interface BPReading {
    id?: number;
    log_date: string;
    reading_num: number;
    time_taken: string;
    systolic: number | null;
    diastolic: number | null;
    notes: string;
  }

  interface TrendDay {
    date: string;
    readings: BPReading[];
    restingHr: number | null;
  }

  let today = $state(new Date().toISOString().split('T')[0]);
  let selectedDate = $state(today);

  let bpForms = $state<BPReading[]>([
    { log_date: today, reading_num: 1, time_taken: '', systolic: null, diastolic: null, notes: '' },
    { log_date: today, reading_num: 2, time_taken: '', systolic: null, diastolic: null, notes: '' },
    { log_date: today, reading_num: 3, time_taken: '', systolic: null, diastolic: null, notes: '' },
  ]);

  let savedStates = $state([false, false, false]);
  let dailyLog = $state<any>(null);
  let trendDays = $state<TrendDay[]>([]);

  onMount(() => {
    loadAll();
  });

  async function loadAll() {
    await Promise.all([loadBP(), loadDailyLog(), loadTrend()]);
  }

  async function loadBP() {
    try {
      const readings: BPReading[] = await invoke<BPReading[]>('get_bp_for_date', { date: selectedDate });
      for (let i = 0; i < 3; i++) {
        const num = i + 1;
        const existing = readings.find(r => r.reading_num === num);
        bpForms[i] = {
          log_date: selectedDate,
          reading_num: num,
          time_taken: existing?.time_taken ?? '',
          systolic: existing?.systolic ?? null,
          diastolic: existing?.diastolic ?? null,
          notes: existing?.notes ?? '',
        };
      }
    } catch (e) {
      console.error('Error loading BP readings:', e);
    }
  }

  async function loadDailyLog() {
    try {
      const logs: any[] = await invoke<any[]>('list_daily_logs', { limit: 30, offset: 0 });
      dailyLog = logs.find(l => l.log_date === selectedDate) || null;
    } catch (e) {
      console.error('Error loading daily logs:', e);
    }
  }

  async function loadTrend() {
    try {
      const dates: string[] = [];
      const baseDate = new Date();
      for (let i = 6; i >= 0; i--) {
        const d = new Date(baseDate);
        d.setDate(d.getDate() - i);
        dates.push(d.toISOString().split('T')[0]);
      }

      const bpResults: BPReading[][] = await Promise.all(
        dates.map(date => invoke<BPReading[]>('get_bp_for_date', { date }))
      );

      const logs: any[] = await invoke<any[]>('list_daily_logs', { limit: 30, offset: 0 });

      trendDays = dates.map((date, i) => ({
        date,
        readings: bpResults[i],
        restingHr: logs.find(l => l.log_date === date)?.ave_resting_hr ?? null,
      }));
    } catch (e) {
      console.error('Error loading trend:', e);
    }
  }

  function onDateChange() {
    for (const form of bpForms) {
      form.log_date = selectedDate;
    }
    loadBP();
    loadDailyLog();
  }

  async function saveReading(num: number) {
    const form = bpForms.find(f => f.reading_num === num);
    if (!form) return;
    try {
      await invoke('upsert_bp', {
        bp: {
          log_date: selectedDate,
          reading_num: num,
          time_taken: form.time_taken,
          systolic: form.systolic,
          diastolic: form.diastolic,
          notes: form.notes,
        },
      });
      savedStates[num - 1] = true;
      setTimeout(() => savedStates[num - 1] = false, 2000);
      loadDailyLog();
    } catch (e) {
      console.error('Error saving BP reading:', e);
    }
  }

  function avgSystolic(): number | null {
    const saved = bpForms.filter(f => f.systolic != null && f.diastolic != null);
    if (saved.length === 0) return null;
    const sum = saved.reduce((a, f) => a + (f.systolic ?? 0), 0);
    return Math.round(sum / saved.length);
  }

  function avgDiastolic(): number | null {
    const saved = bpForms.filter(f => f.systolic != null && f.diastolic != null);
    if (saved.length === 0) return null;
    const sum = saved.reduce((a, f) => a + (f.diastolic ?? 0), 0);
    return Math.round(sum / saved.length);
  }

  function formatReading(readings: BPReading[], num: number): string {
    const r = readings.find(r => r.reading_num === num);
    if (!r || r.systolic == null || r.diastolic == null) return '---';
    return `${r.systolic}/${r.diastolic}`;
  }

  function formatDate(dateStr: string): string {
    const d = new Date(dateStr + 'T00:00:00');
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  }
</script>

<h1>Cardio</h1>

<div class="date-picker">
  <label for="date">Date</label>
  <input type="date" id="date" bind:value={selectedDate} onchange={onDateChange} />
</div>

<div class="reading-cards">
  {#each bpForms as form, i}
    <div class="card reading-card">
      <h3>Reading #{form.reading_num}</h3>

      <div class="field">
        <label>Time (24h)</label>
        <input type="text" bind:value={form.time_taken} placeholder="e.g. 07:30" />
      </div>

      <div class="bp-inputs">
        <div class="field">
          <label>Systolic</label>
          <input type="number" bind:value={form.systolic} placeholder="120" />
        </div>
        <div class="field">
          <label>Diastolic</label>
          <input type="number" bind:value={form.diastolic} placeholder="80" />
        </div>
      </div>

      <div class="field">
        <label>Notes</label>
        <input type="text" bind:value={form.notes} placeholder="Optional notes" />
      </div>

      <button class="save-btn" onclick={() => saveReading(form.reading_num)}>
        {savedStates[i] ? '✓ Saved!' : 'Save Reading'}
      </button>
    </div>
  {/each}
</div>

<div class="card summary-card">
  <h3>Today's BP Average</h3>
  <div class="bp-average">
    <span class="sys">{avgSystolic() ?? '---'}</span>
    <span class="separator">/</span>
    <span class="dia">{avgDiastolic() ?? '---'}</span>
    <span class="unit">mmHg</span>
  </div>
  {#if bpForms.some(f => f.systolic != null && f.diastolic != null)}
    <p class="reading-count">Average of {bpForms.filter(f => f.systolic != null && f.diastolic != null).length} reading(s)</p>
  {:else}
    <p class="reading-count">No readings saved yet</p>
  {/if}
</div>

<h2>Heart Rate</h2>
<div class="hr-section">
  <div class="card metric-card">
    <h4>Resting HR</h4>
    <div class="metric-value">
      <span class="value">{dailyLog?.ave_resting_hr ?? '---'}</span>
      <span class="unit">bpm</span>
    </div>
  </div>
  <div class="card metric-card">
    <h4>Average HR</h4>
    <div class="metric-value">
      <span class="value">{dailyLog?.ave_hr ?? '---'}</span>
      <span class="unit">bpm</span>
    </div>
  </div>
</div>

<h2>7-Day Trend</h2>
<div class="trend-table-wrapper">
  <table class="trend-table">
    <thead>
      <tr>
        <th>Date</th>
        <th>Reading 1</th>
        <th>Reading 2</th>
        <th>Reading 3</th>
        <th>Resting HR</th>
      </tr>
    </thead>
    <tbody>
      {#each trendDays as day, i}
        <tr>
          <td class="date-cell">{formatDate(day.date)}</td>
          <td>{formatReading(day.readings, 1)}</td>
          <td>{formatReading(day.readings, 2)}</td>
          <td>{formatReading(day.readings, 3)}</td>
          <td class="hr-cell">{day.restingHr ?? '---'}</td>
        </tr>
        {#if i < trendDays.length - 1}
          <tr class="divider-row"><td colspan="5"><div class="divider"></div></td></tr>
        {/if}
      {/each}
    </tbody>
  </table>
</div>

<style>
  h1 { margin-bottom: 4px; }
  h2 { margin-top: 32px; margin-bottom: 12px; font-size: 18px; }

  .date-picker {
    margin-bottom: 24px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .date-picker label { font-size: 13px; font-weight: 600; color: #555; }
  :global(.dark) .date-picker label { color: #bbb; }
  .date-picker input {
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 14px;
    font-family: inherit;
    background: #fff;
    color: #333;
  }
  :global(.dark) .date-picker input { background: #1e2a45; border-color: #444; color: #e0e0e0; }

  .reading-cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
    margin-bottom: 24px;
  }

  .card {
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  }
  :global(.dark) .card { background: #1e2a45; }

  .reading-card h3 { margin: 0 0 16px 0; font-size: 15px; }

  .field { display: flex; flex-direction: column; gap: 4px; margin-bottom: 12px; }
  .field label { font-size: 12px; font-weight: 600; color: #555; }
  :global(.dark) .field label { color: #bbb; }
  .field input {
    padding: 8px 10px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
    font-family: inherit;
    background: #fff;
    color: #333;
    width: 100%;
    box-sizing: border-box;
  }
  :global(.dark) .field input { background: #2a3a5c; border-color: #444; color: #e0e0e0; }

  .bp-inputs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .save-btn {
    padding: 10px 20px;
    background: #1976d2;
    color: #fff;
    border: none;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    width: 100%;
    margin-top: 4px;
  }
  .save-btn:hover { background: #1565c0; }

  .summary-card { margin-bottom: 8px; max-width: 400px; }
  .summary-card h3 { margin: 0 0 12px 0; font-size: 15px; }

  .bp-average {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .bp-average .sys { font-size: 36px; font-weight: 700; }
  .bp-average .separator { font-size: 28px; color: #888; }
  .bp-average .dia { font-size: 36px; font-weight: 700; }
  .bp-average .unit { font-size: 14px; color: #888; margin-left: 4px; }

  .reading-count { font-size: 13px; color: #888; margin-top: 8px; }

  .hr-section {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    max-width: 400px;
  }

  .metric-card h4 { margin: 0 0 8px 0; font-size: 13px; color: #555; }
  :global(.dark) .metric-card h4 { color: #bbb; }

  .metric-value { display: flex; align-items: baseline; gap: 6px; }
  .metric-value .value { font-size: 32px; font-weight: 700; }
  .metric-value .unit { font-size: 14px; color: #888; }

  .trend-table-wrapper { overflow-x: auto; }

  .trend-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  .trend-table th {
    text-align: left;
    padding: 10px 12px;
    font-size: 12px;
    font-weight: 600;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 2px solid #eee;
  }
  :global(.dark) .trend-table th { border-bottom-color: #333; color: #999; }
  .trend-table td {
    padding: 10px 12px;
    color: #333;
  }
  :global(.dark) .trend-table td { color: #e0e0e0; }
  .trend-table .date-cell { font-weight: 600; }
  .trend-table .hr-cell { color: #888; }

  .divider-row td { padding: 0 12px; }
  .divider {
    height: 1px;
    background: #eee;
  }
  :global(.dark) .divider { background: #333; }
</style>