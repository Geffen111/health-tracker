<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let today = $state(new Date().toISOString().split('T')[0]);
  let selectedDate = $state(today);
  let saved = $state(false);

  let rosteredHours = $state<number>(7.6);
  let sickLeaveHours = $state<number>(0);
  let officeHours = $state<number>(7.6);
  let wfhHours = $state<number>(0);

  let logs = $state<any[]>([]);
  let monthlyLoading = $state(true);

  onMount(async () => {
    await loadDate(selectedDate);
    await loadMonthly();
  });

  async function loadDate(date: string) {
    try {
      const existing: any = await invoke('get_daily_log', { date });
      if (existing) {
        rosteredHours = existing.rostered_hours ?? 7.6;
        sickLeaveHours = existing.sick_leave_hours ?? 0;
        officeHours = existing.office_hours ?? 7.6;
        wfhHours = existing.wfh_hours ?? 0;
      } else {
        rosteredHours = 7.6;
        sickLeaveHours = 0;
        officeHours = 7.6;
        wfhHours = 0;
      }
    } catch {
      rosteredHours = 7.6;
      sickLeaveHours = 0;
      officeHours = 7.6;
      wfhHours = 0;
    }
  }

  async function onDateChange() {
    await loadDate(selectedDate);
  }

  function buildFullLog() {
    return {
      log_date: selectedDate,
      day_name: null,
      fatigue_desc: null,
      fatigue_rating: null,
      headache_desc: null,
      headache_rating: null,
      headache_duration_hours: null,
      other_symptoms: null,
      my_sleep_rating: null,
      phone_sleep_rating: null,
      sleep_avg: null,
      sleep_time_head_on_pillow: null,
      sleep_actual_asleep: null,
      sleep_rem: null,
      sleep_deep: null,
      sleep_awake: null,
      steps: null,
      activity_calories: null,
      ave_resting_hr: null,
      ave_hr: null,
      rostered_hours: rosteredHours,
      sick_leave_hours: sickLeaveHours,
      office_hours: officeHours,
      wfh_hours: wfhHours,
      alcohol_std_drinks: null,
      multivitamin: null,
      vitamin_c: null,
      add_meds: null,
      compression_socks: null,
      notes: null
    };
  }

  async function save() {
    const log = buildFullLog();
    await invoke('upsert_daily_log', { log });
    saved = true;
    setTimeout(() => saved = false, 2000);
    await loadMonthly();
  }

  async function loadMonthly() {
    monthlyLoading = true;
    try {
      logs = await invoke('list_daily_logs', { limit: 30, offset: 0 }) as any[];
    } catch (e) {
      console.error('Error loading logs:', e);
    } finally {
      monthlyLoading = false;
    }
  }

  function getWeekStart(dateStr: string): string {
    const d = new Date(dateStr + 'T00:00:00');
    const day = d.getDay();
    d.setDate(d.getDate() - day);
    return d.toISOString().split('T')[0];
  }

  let weekGroups = $derived.by(() => {
    const sorted = [...logs].sort((a, b) => a.log_date.localeCompare(b.log_date));
    const groups: { weekStart: string; logs: any[] }[] = [];
    let currentWeek = '';
    let currentGroup: any[] = [];

    for (const log of sorted) {
      const ws = getWeekStart(log.log_date);
      if (ws !== currentWeek && currentGroup.length > 0) {
        groups.push({ weekStart: currentWeek, logs: currentGroup });
        currentGroup = [];
      }
      currentWeek = ws;
      currentGroup.push(log);
    }
    if (currentGroup.length > 0) {
      groups.push({ weekStart: currentWeek, logs: currentGroup });
    }
    return groups;
  });

  function totalHours(field: string, src: any[]): number {
    return src.reduce((sum, l) => sum + (l[field] ?? 0), 0);
  }

  function formatDate(dateStr: string): string {
    const d = new Date(dateStr + 'T00:00:00');
    return d.toLocaleDateString('en-US', { weekday: 'short', month: 'short', day: 'numeric' });
  }

  function fmt(val: number | null | undefined): string {
    if (val == null) return '—';
    return val.toFixed(2);
  }
</script>

<h1>Work Hours</h1>

<div class="date-picker">
  <label for="date-input">Date</label>
  <input id="date-input" type="date" bind:value={selectedDate} onchange={onDateChange} />
</div>

<div class="card form-card">
  <h2>Work Hours Entry</h2>
  <div class="form-grid">
    <div class="field">
      <label for="rostered">Rostered Hours</label>
      <input id="rostered" type="number" step="0.25" min="0" bind:value={rosteredHours} />
    </div>
    <div class="field">
      <label for="sick">Sick Leave Hours</label>
      <input id="sick" type="number" step="0.25" min="0" bind:value={sickLeaveHours} />
    </div>
    <div class="field">
      <label for="office">Office Hours</label>
      <input id="office" type="number" step="0.25" min="0" bind:value={officeHours} />
    </div>
    <div class="field">
      <label for="wfh">WFH Hours</label>
      <input id="wfh" type="number" step="0.25" min="0" bind:value={wfhHours} />
    </div>
  </div>
  <p class="notes-hint">Day notes now live in <strong>Daily Log → Other Daily Notes</strong>.</p>
  <button class="save-btn" onclick={save}>
    {saved ? '✓ Saved!' : 'Save'}
  </button>
</div>

<div class="card summary-card">
  <h2>Daily Summary — {formatDate(selectedDate)}</h2>
  <div class="summary-row">
    <div class="summary-item">
      <span class="summary-label">Total Worked</span>
      <span class="summary-value">{(officeHours + wfhHours).toFixed(2)}h</span>
    </div>
    <div class="summary-item">
      <span class="summary-label">Rostered</span>
      <span class="summary-value">{fmt(rosteredHours)}h</span>
    </div>
    <div class="summary-item">
      <span class="summary-label">Sick Leave</span>
      <span class="summary-value">{fmt(sickLeaveHours)}h</span>
    </div>
    <div class="summary-item">
      <span class="summary-label">Status</span>
      <span
        class="status-badge"
        class:green={officeHours + wfhHours >= rosteredHours && sickLeaveHours === 0}
        class:amber={officeHours + wfhHours < rosteredHours && officeHours + wfhHours > 0 && sickLeaveHours === 0}
        class:red={sickLeaveHours > 0}
      >
        {sickLeaveHours > 0 ? 'Sick Leave' : officeHours + wfhHours >= rosteredHours ? '✔ Full Day' : 'Partial Day'}
      </span>
    </div>
  </div>
</div>

<div class="card table-card">
  <h2>Monthly View</h2>
  {#if monthlyLoading}
    <p class="loading">Loading...</p>
  {:else if logs.length === 0}
    <p class="empty">No work data recorded yet.</p>
  {:else}
    <div class="table-scroll">
      <table>
        <thead>
          <tr>
            <th>Date</th>
            <th>Rostered</th>
            <th>Office</th>
            <th>WFH</th>
            <th>Sick</th>
            <th>Notes</th>
          </tr>
        </thead>
        <tbody>
          {#each weekGroups as group}
            {#each group.logs as log}
              <tr>
                <td class="date-cell">{formatDate(log.log_date)}</td>
                <td>{fmt(log.rostered_hours)}</td>
                <td>{fmt(log.office_hours)}</td>
                <td>{fmt(log.wfh_hours)}</td>
                <td>{fmt(log.sick_leave_hours)}</td>
                <td class="notes-cell">{log.notes || ''}</td>
              </tr>
            {/each}
            <tr class="week-total">
              <td class="week-label">Week Total</td>
              <td>{fmt(totalHours('rostered_hours', group.logs))}</td>
              <td>{fmt(totalHours('office_hours', group.logs))}</td>
              <td>{fmt(totalHours('wfh_hours', group.logs))}</td>
              <td>{fmt(totalHours('sick_leave_hours', group.logs))}</td>
              <td></td>
            </tr>
          {/each}
          <tr class="grand-total">
            <td>Total</td>
            <td>{fmt(totalHours('rostered_hours', logs))}</td>
            <td>{fmt(totalHours('office_hours', logs))}</td>
            <td>{fmt(totalHours('wfh_hours', logs))}</td>
            <td>{fmt(totalHours('sick_leave_hours', logs))}</td>
            <td></td>
          </tr>
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  h1 { margin-bottom: 20px; }

  .date-picker {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 20px;
  }
  .date-picker label { font-size: 14px; font-weight: 600; color: #555; }
  :global(.dark) .date-picker label { color: #bbb; }

  input[type="date"] {
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 14px;
    background: #fff;
  }
  :global(.dark) input[type="date"] {
    background: #2a3a5c;
    border-color: #444;
    color: #e0e0e0;
  }

  .card {
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    margin-bottom: 20px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  }
  :global(.dark) .card { background: #1e2a45; }

  .form-card { max-width: 700px; }
  .notes-hint { font-size: 12px; color: #888; margin-top: 14px; margin-bottom: 0; }
  :global(.dark) .notes-hint { color: #999; }
  .summary-card { max-width: 700px; }

  .card h2 {
    font-size: 16px;
    margin-bottom: 16px;
    font-weight: 600;
  }

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label { font-size: 13px; font-weight: 600; color: #555; }
  :global(.dark) label { color: #bbb; }

  input[type="number"] {
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
    width: 100%;
  }
  :global(.dark) input[type="number"] {
    background: #2a3a5c;
    border-color: #444;
    color: #e0e0e0;
  }

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
    margin-top: 16px;
  }
  .save-btn:hover { background: #1565c0; }

  .summary-row {
    display: flex;
    gap: 28px;
    align-items: center;
    flex-wrap: wrap;
  }

  .summary-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .summary-label {
    font-size: 12px;
    color: #888;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  :global(.dark) .summary-label { color: #999; }

  .summary-value {
    font-size: 22px;
    font-weight: 700;
  }

  .status-badge {
    padding: 4px 12px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
  }

  .green { background: #e8f5e9; color: #2e7d32; }
  :global(.dark) .green { background: #1b3d1f; color: #66bb6a; }
  .amber { background: #fff3e0; color: #e65100; }
  :global(.dark) .amber { background: #3d2b1a; color: #ffb74d; }
  .red { background: #ffebee; color: #c62828; }
  :global(.dark) .red { background: #3d1a1a; color: #ef5350; }

  .loading, .empty { color: #888; text-align: center; padding: 16px; }

  .table-scroll { overflow-x: auto; }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th {
    text-align: left;
    padding: 8px 10px;
    border-bottom: 2px solid #eee;
    font-weight: 600;
    font-size: 12px;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    white-space: nowrap;
  }
  :global(.dark) th { border-color: #333; color: #999; }

  td {
    padding: 8px 10px;
    border-bottom: 1px solid #f0f0f0;
    white-space: nowrap;
  }
  :global(.dark) td { border-color: #2a2a4a; }

  .date-cell { font-weight: 600; }
  .notes-cell { max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #888; }
  :global(.dark) .notes-cell { color: #999; }

  .week-total td {
    border-top: 2px solid #1976d2;
    border-bottom: 2px solid #1976d2;
    font-weight: 600;
    background: #f8faff;
  }
  :global(.dark) .week-total td {
    background: #1a2a4a;
  }

  .week-label { font-size: 11px; color: #1976d2; text-transform: uppercase; letter-spacing: 0.5px; }
  :global(.dark) .week-label { color: #64b5f6; }

  .grand-total td {
    border-top: 3px double #333;
    font-weight: 700;
    font-size: 14px;
  }
  :global(.dark) .grand-total td { border-color: #888; }
</style>