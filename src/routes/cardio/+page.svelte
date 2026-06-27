<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateLong } from '$lib/formatDate';

  let today = $state(new Date().toISOString().split('T')[0]);
  let selectedDate = $state(today);
  let bpReadings = $state<any[]>([]);
  let dailyLog = $state<any>(null);
  let darkMode = $state(false);
  let banner = $state(false);
  let calDays = $state<number | null>(null);

  let nTime = $state('');
  let nSys = $state('');
  let nDia = $state('');

  onMount(() => { loadAll(); });

  async function loadAll() {
    await Promise.all([loadBP(), loadDailyLog(), loadCalDays()]);
  }

  async function loadBP() {
    try {
      bpReadings = await invoke('get_bp_for_date', { date: selectedDate });
    } catch (e) { console.error('Error loading BP:', e); }
  }

  async function loadDailyLog() {
    try {
      const logs: any[] = await invoke('list_daily_logs', { limit: 30, offset: 0 });
      dailyLog = logs.find((l: any) => l.log_date === selectedDate) || null;
    } catch (e) { console.error('Error loading daily logs:', e); }
  }

  async function loadCalDays() {
    try {
      calDays = await invoke('days_since_calibration');
    } catch {}
  }

  function toggleTheme() {
    darkMode = !darkMode;
    document.documentElement.classList.toggle('dark', darkMode);
  }

  function prevDay() {
    const d = new Date(selectedDate + 'T00:00:00');
    d.setDate(d.getDate() - 1);
    selectedDate = d.toISOString().split('T')[0];
    loadBP();
  }

  function nextDay() {
    const d = new Date(selectedDate + 'T00:00:00');
    d.setDate(d.getDate() + 1);
    selectedDate = d.toISOString().split('T')[0];
    loadBP();
  }

  async function addReading() {
    if (!nSys || !nDia) return;
    try {
      const nextNum = bpReadings.length > 0 ? Math.max(...bpReadings.map((r: any) => r.reading_num)) + 1 : 1;
      await invoke('upsert_bp', {
        bp: {
          log_date: selectedDate,
          reading_num: nextNum,
          time_taken: nTime || null,
          systolic: parseInt(nSys),
          diastolic: parseInt(nDia),
          notes: null,
        },
      });
      nTime = ''; nSys = ''; nDia = '';
      await loadBP();
    } catch (e) { console.error('Error saving BP:', e); }
  }

  async function deleteReading(readingNum: number) {
    try {
      await invoke('upsert_bp', {
        bp: {
          log_date: selectedDate,
          reading_num: readingNum,
          time_taken: null,
          systolic: null,
          diastolic: null,
          notes: 'DELETED',
        },
      });
      await loadBP();
    } catch (e) { console.error('Error deleting BP:', e); }
  }

  let avgSys = $derived.by(() => {
    const valid = bpReadings.filter((r: any) => r.systolic != null && r.diastolic != null);
    return valid.length > 0 ? Math.round(valid.reduce((a: number, r: any) => a + r.systolic, 0) / valid.length) : null;
  });
  let avgDia = $derived.by(() => {
    const valid = bpReadings.filter((r: any) => r.systolic != null && r.diastolic != null);
    return valid.length > 0 ? Math.round(valid.reduce((a: number, r: any) => a + r.diastolic, 0) / valid.length) : null;
  });

  function tagFor(sys: number, dia: number): { tag: string; dot: string } {
    if (sys >= 140 || dia >= 90) return { tag: 'Elevated', dot: 'var(--amber)' };
    if (sys < 100 || dia < 65) return { tag: 'Low', dot: 'var(--peri)' };
    return { tag: 'Normal', dot: 'var(--accent)' };
  }

  let overdue = $derived(calDays != null && calDays >= 30);
  let calPct = $derived(calDays != null ? Math.min(100, Math.round((calDays / 30) * 100)) : 0);

  async function logCalibration() {
    try {
      await invoke('log_watch_calibration', {});
      calDays = 0;
      banner = true;
      setTimeout(() => banner = false, 3000);
    } catch (e) { console.error('Error logging calibration:', e); }
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Cardio</div>
    <div class="page-subtitle">Blood pressure, heart rate &amp; watch calibration</div>
  </div>
  <div class="header-actions">
    <div class="day-nav">
      <button class="day-arrow" onclick={prevDay} aria-label="Previous day">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 6l-6 6 6 6"/></svg>
      </button>
      <span class="day-label">{formatDateLong(selectedDate)}</span>
      <button class="day-arrow" onclick={nextDay} disabled={selectedDate === today} aria-label="Next day">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
      </button>
    </div>
    <button class="theme-btn" onclick={toggleTheme} aria-label="Toggle theme">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/></svg>
    </button>
  </div>
</div>

{#if banner}
  <div class="banner">
    <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg>
    <span>Watch calibration logged — recorded just now.</span>
    <button class="banner-dismiss" onclick={() => banner = false}>Dismiss</button>
  </div>
{/if}

<div class="bp-hr-row">
  <div class="bp-card">
    <div class="bp-header">
      <div>
        <div class="card-heading">Blood pressure</div>
        <div class="card-subtitle">{bpReadings.length} reading{bpReadings.length !== 1 ? 's' : ''} today · daily average</div>
      </div>
      <div class="bp-avg">
        <span class="bp-avg-sys">{avgSys ?? '---'}</span>
        <span class="bp-avg-sep">/</span>
        <span class="bp-avg-dia">{avgDia ?? '---'}</span>
        <span class="bp-avg-unit">mmHg</span>
      </div>
    </div>
    <div class="bp-list">
      {#each bpReadings as r}
        {@const t = tagFor(r.systolic, r.diastolic)}
        <div class="bp-row">
          <span class="bp-time">{r.time_taken ?? '--:--'}</span>
          <span class="bp-dot" style="background:{t.dot};"></span>
          <span class="bp-values"><strong>{r.systolic}/{r.diastolic}</strong> <span class="bp-unit">mmHg</span></span>
          <span class="bp-tag">{t.tag}</span>
          <button class="bp-delete" onclick={() => deleteReading(r.reading_num)} aria-label="Delete reading">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
          </button>
        </div>
      {/each}
      <div class="bp-add">
        <input bind:value={nTime} placeholder="HH:MM" class="bp-input sm" />
        <input bind:value={nSys} placeholder="Sys" class="bp-input xs" />
        <span class="bp-slash">/</span>
        <input bind:value={nDia} placeholder="Dia" class="bp-input xs" />
        <button class="add-reading-btn" onclick={addReading}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
          Add reading
        </button>
      </div>
    </div>
  </div>

  <div class="hr-card">
    <div class="hr-header">
      <span class="card-heading">Heart rate</span>
      <span class="hr-badge">Watch-synced</span>
    </div>
    <div class="hr-grid">
      <div class="hr-tile">
        <div class="hr-tile-label">Resting</div>
        <div class="hr-tile-val">{dailyLog?.ave_resting_hr ?? '—'}<span class="hr-unit"> bpm</span></div>
      </div>
      <div class="hr-tile">
        <div class="hr-tile-label">Average</div>
        <div class="hr-tile-val">{dailyLog?.ave_hr ?? '—'}<span class="hr-unit"> bpm</span></div>
      </div>
      <div class="hr-tile">
        <div class="hr-tile-label">Daily min</div>
        <div class="hr-tile-val">{dailyLog?.hr_min ?? '—'}<span class="hr-unit"> bpm</span></div>
      </div>
      <div class="hr-tile">
        <div class="hr-tile-label">Daily max</div>
        <div class="hr-tile-val">{dailyLog?.hr_max ?? '—'}<span class="hr-unit"> bpm</span></div>
      </div>
    </div>
    <div class="hr-note">Min &amp; max come from the watch's continuous monitoring; resting and average are the daily figures.</div>
  </div>
</div>

<div class="cal-card">
  <div class="cal-icon">
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="8"/><path d="M12 8v4l2.5 2.5"/></svg>
  </div>
  <div class="cal-info">
    <div class="card-heading">Watch calibration</div>
    <div class="cal-status">
      {calDays != null ? `Last logged ${calDays} day${calDays === 1 ? '' : 's'} ago${overdue ? ' — overdue' : ` · next due in ${30 - calDays} days`}` : 'No calibration logged yet'}
    </div>
    <div class="cal-bar-track">
      <div class="cal-bar-fill" style="width:{calPct}%;background:{overdue ? 'var(--amber)' : 'var(--accent)'};"></div>
    </div>
  </div>
  <div class="cal-right">
    <span class="cal-badge" style="color:{overdue ? 'var(--amber-fg)' : 'var(--accent-fg)'};background:{overdue ? 'var(--amber-soft)' : 'var(--accent-soft)'};">{overdue ? 'Recalibration due' : 'On track'}</span>
    <button class="cal-btn" onclick={logCalibration}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14M5 12h14"/></svg>
      Log calibration
    </button>
  </div>
</div>

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:22px; gap:16px; flex-wrap:wrap; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }
  .header-actions { display:flex; align-items:center; gap:10px; }
  .day-nav { display:flex; align-items:center; gap:2px; background:var(--card); border:1px solid var(--border); border-radius:999px; padding:4px; box-shadow:var(--shadow); }
  .day-arrow { width:30px;height:30px;border-radius:50%;border:none;background:transparent;color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .day-arrow:disabled { color:var(--tm); cursor:not-allowed; }
  .day-label { font-weight:700; font-size:13px; padding:0 6px; min-width:108px; text-align:center; }
  .theme-btn { width:36px;height:36px;border-radius:50%;border:1px solid var(--border);background:var(--card);color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer; }

  .banner { display:flex;align-items:center;gap:11px;background:var(--accent-soft);border:1px solid var(--border);border-radius:14px;padding:12px 16px;margin-bottom:16px; }
  .banner span { font-size:13px;color:var(--accent-fg);font-weight:600;flex:1; }
  .banner-dismiss { border:none;background:transparent;color:var(--ts);cursor:pointer;font-size:13px;font-weight:700; }

  .bp-hr-row { display:grid; grid-template-columns:1.5fr 1fr; gap:16px; margin-bottom:16px; }

  .bp-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:18px; }
  .bp-header { display:flex; justify-content:space-between; align-items:flex-end; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12px; color:var(--ts); margin-top:2px; }
  .bp-avg { text-align:right; }
  .bp-avg-sys, .bp-avg-dia { font-family:'Source Serif 4',serif; font-size:34px; font-weight:600; color:var(--tp); letter-spacing:-.01em; font-variant-numeric:tabular-nums; }
  .bp-avg-sep { color:var(--ts); font-size:24px; }
  .bp-avg-unit { font-size:13px; color:var(--tm); }

  .bp-list { display:flex; flex-direction:column; border:1px solid var(--border); border-radius:14px; overflow:hidden; }
  .bp-row { display:flex; align-items:center; gap:14px; padding:12px 16px; border-bottom:1px solid var(--border); }
  .bp-time { font-size:12.5px; color:var(--ts); font-variant-numeric:tabular-nums; width:48px; font-weight:600; }
  .bp-dot { width:8px;height:8px;border-radius:50%;flex-shrink:0; }
  .bp-values { flex:1; font-size:14px; color:var(--tp); font-variant-numeric:tabular-nums; }
  .bp-values strong { font-weight:600; }
  .bp-unit { color:var(--tm); font-size:12px; font-weight:400; }
  .bp-tag { font-size:11.5px; color:var(--tm); }
  .bp-delete { width:26px;height:26px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer; }

  .bp-add { display:flex; align-items:center; gap:8px; padding:12px 14px; background:var(--inset); }
  .bp-input { background:var(--card); border:1px solid var(--border); border-radius:9px; padding:8px; font-size:12.5px; color:var(--tp); text-align:center; font-variant-numeric:tabular-nums; }
  .bp-input.sm { width:64px; }
  .bp-input.xs { width:56px; }
  .bp-slash { color:var(--tm); }
  .add-reading-btn { margin-left:auto; display:inline-flex; align-items:center; gap:6px; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:8px 15px; font-size:12.5px; font-weight:700; cursor:pointer; white-space:nowrap; }

  .hr-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:16px; }
  .hr-header { display:flex; justify-content:space-between; align-items:center; }
  .hr-badge { font-size:10.5px; font-weight:700; color:var(--accent-fg); background:var(--accent-soft); padding:3px 9px; border-radius:999px; }
  .hr-grid { display:grid; grid-template-columns:1fr 1fr; gap:12px; }
  .hr-tile { background:var(--inset); border-radius:13px; padding:13px 14px; }
  .hr-tile-label { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .hr-tile-val { font-family:'Source Serif 4',serif; font-size:25px; font-weight:600; color:var(--tp); }
  .hr-unit { font-size:12px; color:var(--tm); }
  .hr-note { font-size:11.5px; color:var(--ts); line-height:1.5; }

  .cal-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; align-items:center; gap:24px; flex-wrap:wrap; }
  .cal-icon { width:46px;height:46px;border-radius:13px;background:var(--accent-soft);display:flex;align-items:center;justify-content:center;flex-shrink:0;color:var(--accent); }
  .cal-info { flex:1; min-width:200px; }
  .cal-status { font-size:12.5px; color:var(--ts); margin-top:2px; }
  .cal-bar-track { height:7px; border-radius:999px; background:var(--inset); overflow:hidden; margin-top:11px; max-width:320px; }
  .cal-bar-fill { height:100%; border-radius:999px; }
  .cal-right { display:flex; flex-direction:column; align-items:flex-end; gap:10px; }
  .cal-badge { font-size:11.5px; font-weight:700; padding:4px 11px; border-radius:999px; }
  .cal-btn { display:inline-flex; align-items:center; gap:7px; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:10px 18px; font-size:13px; font-weight:700; cursor:pointer; }
</style>