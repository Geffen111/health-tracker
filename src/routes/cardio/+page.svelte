<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateLong, formatDateShort, todayISO, shiftISO } from '$lib/formatDate';
  import Chart from '$lib/Chart.svelte';

  let today = $state(todayISO());
  let nowTime = new Date().toTimeString().slice(0, 5);
  let selectedDate = $state(today);
  let bpReadings = $state<any[]>([]);
  // Heart rate is a full-day synced metric — when viewing today it's still
  // incomplete, so the HR tiles show the most recent complete day (yesterday).
  let hrLog = $state<any>(null);
  let hrDate = $derived(selectedDate === today ? shiftISO(today, -1) : selectedDate);
  // Resting HR isn't reliably populated by health sync, so it's manually editable.
  let restingEdit = $state<number | null>(null);
  let restingSaved = $state(false);
  let banner = $state(false);
  let calDays = $state<number | null>(null);
  let lastCal = $state<any>(null);
  let calDate = $state(today);
  let calTime = $state(nowTime);

  let nTime = $state('');
  let nSys = $state('');
  let nDia = $state('');

  // History chart: a couplet selector + range toggle. Each couplet's two series
  // sit on a left and right y-axis so differing scales read clearly.
  let histDays = $state(30);
  let histMetric = $state<'bp' | 'minmax' | 'avg'>('bp');
  let histLogs = $state<any[]>([]);   // daily_logs (HR fields), oldest first
  let bpHistory = $state<any[]>([]);  // daily-averaged BP, oldest first

  const HIST: Record<string, { label: string; src: 'bp' | 'hr'; unit: string; a: { key: string; label: string; color: string }; b: { key: string; label: string; color: string } }> = {
    bp: { label: 'Blood pressure', src: 'bp', unit: 'mmHg', a: { key: 'avg_systolic', label: 'Systolic', color: 'var(--red)' }, b: { key: 'avg_diastolic', label: 'Diastolic', color: 'var(--peri)' } },
    minmax: { label: 'Min / Max HR', src: 'hr', unit: 'bpm', a: { key: 'hr_min', label: 'Min HR', color: 'var(--peri)' }, b: { key: 'hr_max', label: 'Max HR', color: 'var(--red)' } },
    avg: { label: 'Avg / Resting HR', src: 'hr', unit: 'bpm', a: { key: 'ave_hr', label: 'Avg HR', color: 'var(--accent)' }, b: { key: 'ave_resting_hr', label: 'Resting HR', color: 'var(--amber)' } },
  };

  let histCfg = $derived(HIST[histMetric]);
  let histRows = $derived.by(() => {
    const src = histCfg.src === 'bp' ? bpHistory : histLogs;
    const sorted = [...src].sort((a, b) => a.log_date.localeCompare(b.log_date));
    return sorted.slice(-histDays);
  });
  let histLabels = $derived(histRows.map((r: any) => formatDateShort(r.log_date)));

  // BP and Min/Max HR are ranges, so each day reads best as a vertical connector with
  // a hollow marker at each end (low & high) rather than two horizontal trend lines.
  // Avg/Resting HR aren't a range, so they stay as two independent lines.
  let isRange = $derived(histMetric === 'bp' || histMetric === 'minmax');
  let rangeColor = $derived(histMetric === 'bp' ? 'var(--red)' : 'var(--accent)');

  function lowHigh(r: any): { low: number | null; high: number | null } {
    const v1 = r[histCfg.a.key], v2 = r[histCfg.b.key];
    if (v1 == null || v2 == null) return { low: null, high: null };
    return { low: Math.min(v1, v2), high: Math.max(v1, v2) };
  }

  let histDatasets = $derived.by(() => {
    if (isRange) {
      const color = rangeColor;
      const ring = {
        showLine: false,
        pointRadius: 4,
        pointHoverRadius: 6,
        pointBorderColor: color,
        pointBackgroundColor: 'var(--card)', // hollow centre, theme-aware
        pointBorderWidth: 2,
        borderColor: color,
        backgroundColor: color,
        order: 1,
      };
      return [
        // Vertical connector: a thin floating bar from the day's low to its high.
        {
          type: 'bar',
          label: '_range',
          data: histRows.map((r: any) => { const { low, high } = lowHigh(r); return low != null && high != null ? [low, high] : null; }),
          backgroundColor: color,
          borderWidth: 0,
          barThickness: 3,
          order: 2,
        },
        { type: 'line', label: histMetric === 'bp' ? 'Systolic' : 'Max HR', data: histRows.map((r: any) => lowHigh(r).high), ...ring },
        { type: 'line', label: histMetric === 'bp' ? 'Diastolic' : 'Min HR', data: histRows.map((r: any) => lowHigh(r).low), ...ring },
      ];
    }
    return [
      { label: histCfg.a.label, data: histRows.map((r: any) => r[histCfg.a.key] ?? null), borderColor: histCfg.a.color, backgroundColor: histCfg.a.color },
      { label: histCfg.b.label, data: histRows.map((r: any) => r[histCfg.b.key] ?? null), borderColor: histCfg.b.color, backgroundColor: histCfg.b.color },
    ];
  });
  let histOptions = $derived({
    elements: { point: { radius: 2, hoverRadius: 5 } },
    spanGaps: true,
    interaction: { mode: 'index', intersect: false },
    scales: {
      y: { type: 'linear', grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } } },
      x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 }, maxTicksLimit: 6 } },
    },
    plugins: {
      legend: { display: !isRange, labels: { color: 'var(--ts)', font: { size: 11 }, boxWidth: 10, padding: 12 } },
      // The connector bar is a visual aid, not a data series — keep it out of tooltips.
      tooltip: { filter: (item: any) => item.dataset.label !== '_range' },
    },
  });
  let histHasData = $derived(histDatasets.some((d: any) => (d.data as any[]).some((v: any) => v != null)));

  onMount(() => { loadAll(); });

  async function loadAll() {
    await Promise.all([loadBP(), loadDailyLog(), loadCal(), loadHistory()]);
  }

  async function loadHistory() {
    try {
      [histLogs, bpHistory] = await Promise.all([
        invoke<any[]>('list_daily_logs', { limit: 60, offset: 0 }),
        invoke<any[]>('get_bp_history', { days: 60 }),
      ]);
    } catch (e) { console.error('Error loading cardio history:', e); }
  }

  async function loadBP() {
    try {
      bpReadings = await invoke('get_bp_for_date', { date: selectedDate });
    } catch (e) { console.error('Error loading BP:', e); }
  }

  async function loadDailyLog() {
    try {
      const logs: any[] = await invoke('list_daily_logs', { limit: 30, offset: 0 });
      hrLog = logs.find((l: any) => l.log_date === hrDate) || null;
      // Resting HR follows hrDate too, so all four HR tiles (and the card's
      // "Yesterday" note) refer to the same day.
      restingEdit = hrLog?.ave_resting_hr ?? null;
    } catch (e) { console.error('Error loading daily logs:', e); }
  }

  // Manually save resting HR for the day the HR card shows (hrDate = yesterday when
  // viewing today), keeping it in step with the synced avg/min/max tiles.
  async function saveResting() {
    try {
      const v = restingEdit === null || (restingEdit as any) === '' ? null : Math.round(Number(restingEdit));
      await invoke('upsert_daily_log', { log: { log_date: hrDate, ave_resting_hr: v } });
      await Promise.all([loadDailyLog(), loadHistory()]);
      restingSaved = true;
      setTimeout(() => restingSaved = false, 1500);
    } catch (e) { console.error('Error saving resting HR:', e); }
  }

  async function loadCal() {
    try {
      calDays = await invoke('days_since_calibration');
      const recent: any[] = await invoke('list_watch_calibrations', { limit: 1 });
      lastCal = recent.length > 0 ? recent[0] : null;
    } catch {}
  }

  function prevDay() { selectedDate = shiftISO(selectedDate, -1); loadBP(); loadDailyLog(); }
  function nextDay() { selectedDate = shiftISO(selectedDate, 1); loadBP(); loadDailyLog(); }

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
      await invoke('delete_bp', { logDate: selectedDate, readingNum });
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
      await invoke('log_watch_calibration', {
        calDate: calDate || null,
        calTime: calTime || null,
      });
      await loadCal();
      banner = true;
      setTimeout(() => banner = false, 3000);
    } catch (e) { console.error('Error logging calibration:', e); }
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Cardio</div>
    <div class="page-subtitle">Blood pressure, heart rate &amp; monitor calibration</div>
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
  </div>
</div>

{#if banner}
  <div class="banner">
    <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg>
    <span>Blood pressure monitor calibration logged.</span>
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
        <input type="time" bind:value={nTime} class="bp-input time" />
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
      {#if hrDate !== selectedDate}
        <span class="hr-day">Yesterday · {formatDateShort(hrDate)}</span>
      {/if}
    </div>
    <div class="hr-grid">
      <div class="hr-tile">
        <div class="hr-tile-label">Resting <span class="hr-manual">· manual{restingSaved ? ' · saved' : ''}</span></div>
        <div class="hr-edit">
          <input class="hr-input" type="number" min="0" max="250" bind:value={restingEdit} onchange={saveResting} placeholder="—" aria-label="Resting heart rate" />
          <span class="hr-unit">bpm</span>
        </div>
      </div>
      <div class="hr-tile">
        <div class="hr-tile-label">Average</div>
        <div class="hr-tile-val">{hrLog?.ave_hr ?? '—'}<span class="hr-unit"> bpm</span></div>
      </div>
      <div class="hr-tile">
        <div class="hr-tile-label">Daily min</div>
        <div class="hr-tile-val">{hrLog?.hr_min ?? '—'}<span class="hr-unit"> bpm</span></div>
      </div>
      <div class="hr-tile">
        <div class="hr-tile-label">Daily max</div>
        <div class="hr-tile-val">{hrLog?.hr_max ?? '—'}<span class="hr-unit"> bpm</span></div>
      </div>
    </div>
    <div class="hr-note">Today's heart-rate figures aren't complete until the day ends, so this shows the most recent full day. Min &amp; max come from the watch's continuous monitoring.</div>
  </div>
</div>

<div class="hist-card">
  <div class="hist-header">
    <div>
      <div class="card-heading">History</div>
      <div class="card-subtitle">{histCfg.label} · {histDays} days</div>
    </div>
    <div class="hist-controls">
      <div class="seg">
        <button class="seg-btn" class:active={histMetric === 'bp'} onclick={() => histMetric = 'bp'}>BP</button>
        <button class="seg-btn" class:active={histMetric === 'minmax'} onclick={() => histMetric = 'minmax'}>Min/Max HR</button>
        <button class="seg-btn" class:active={histMetric === 'avg'} onclick={() => histMetric = 'avg'}>Avg/Resting</button>
      </div>
      <div class="seg">
        <button class="seg-btn" class:active={histDays === 14} onclick={() => histDays = 14}>14D</button>
        <button class="seg-btn" class:active={histDays === 30} onclick={() => histDays = 30}>30D</button>
        <button class="seg-btn" class:active={histDays === 60} onclick={() => histDays = 60}>60D</button>
      </div>
    </div>
  </div>
  <div style="height:240px;">
    {#if histHasData}
      <Chart type={isRange ? 'bar' : 'line'} labels={histLabels} datasets={histDatasets} options={histOptions} chartArea="240px" />
    {:else}
      <div class="hist-empty">No {histCfg.label.toLowerCase()} data in this range.</div>
    {/if}
  </div>
</div>

<div class="cal-card">
  <div class="cal-icon">
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="8"/><path d="M12 8v4l2.5 2.5"/></svg>
  </div>
  <div class="cal-info">
    <div class="card-heading">Blood pressure monitor calibration</div>
    <div class="cal-status">
      {#if lastCal}
        Last calibrated: {formatDateLong(lastCal.cal_date)}{lastCal.cal_time ? ` · ${lastCal.cal_time}` : ''}
        {#if calDays != null}<span class="cal-due"> — {overdue ? 'recalibration overdue' : `next due in ${30 - calDays} days`}</span>{/if}
      {:else}
        No calibration logged yet — recommended every 30 days
      {/if}
    </div>
    <div class="cal-bar-track">
      <div class="cal-bar-fill" style="width:{calPct}%;background:{overdue ? 'var(--amber)' : 'var(--accent)'};"></div>
    </div>
  </div>
  <div class="cal-right">
    <div class="cal-entry">
      <input type="date" bind:value={calDate} max={today} class="cal-input" aria-label="Calibration date" />
      <input type="time" bind:value={calTime} class="cal-input" aria-label="Calibration time" />
      <button class="cal-btn" onclick={logCalibration}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14M5 12h14"/></svg>
        Log
      </button>
    </div>
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
  .bp-input.time { width:auto; }
  .bp-input.xs { width:56px; }
  .bp-slash { color:var(--tm); }
  .add-reading-btn { margin-left:auto; display:inline-flex; align-items:center; gap:6px; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:8px 15px; font-size:12.5px; font-weight:700; cursor:pointer; white-space:nowrap; }

  .hr-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:16px; }
  .hr-header { display:flex; justify-content:space-between; align-items:center; }
  .hr-day { font-size:11.5px; color:var(--tm); font-weight:600; }
  .hr-grid { display:grid; grid-template-columns:1fr 1fr; gap:12px; }
  .hr-tile { background:var(--inset); border-radius:13px; padding:13px 14px; }
  .hr-tile-label { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .hr-tile-val { font-family:'Source Serif 4',serif; font-size:25px; font-weight:600; color:var(--tp); }
  .hr-unit { font-size:12px; color:var(--tm); }
  .hr-manual { font-size:9.5px; letter-spacing:0; text-transform:none; font-weight:600; color:var(--tm); }
  .hr-edit { display:flex; align-items:baseline; gap:6px; margin-top:2px; }
  .hr-input { width:70px; background:var(--card); border:1px solid var(--border); border-radius:9px; padding:4px 8px; font-family:'Source Serif 4',serif; font-size:22px; font-weight:600; color:var(--tp); font-variant-numeric:tabular-nums; }
  .hr-input:focus { outline:none; border-color:var(--accent); }
  .hr-note { font-size:11.5px; color:var(--ts); line-height:1.5; }

  .cal-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; align-items:center; gap:24px; flex-wrap:wrap; }
  .cal-icon { width:46px;height:46px;border-radius:13px;background:var(--accent-soft);display:flex;align-items:center;justify-content:center;flex-shrink:0;color:var(--accent); }
  .cal-info { flex:1; min-width:200px; }
  .cal-status { font-size:12.5px; color:var(--ts); margin-top:2px; }
  .cal-bar-track { height:7px; border-radius:999px; background:var(--inset); overflow:hidden; margin-top:11px; max-width:320px; }
  .cal-bar-fill { height:100%; border-radius:999px; }
  .cal-right { display:flex; flex-direction:column; align-items:flex-end; gap:10px; }
  .cal-due { color:var(--tm); }

  .hist-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:20px 22px; box-shadow:var(--shadow); margin-bottom:16px; display:flex; flex-direction:column; gap:16px; }
  .hist-header { display:flex; justify-content:space-between; align-items:flex-start; gap:14px; flex-wrap:wrap; }
  .hist-controls { display:flex; gap:8px; flex-wrap:wrap; }
  .seg { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:999px; padding:3px; gap:2px; }
  .seg-btn { background:transparent; border:none; color:var(--ts); border-radius:999px; padding:6px 12px; font-size:12px; font-weight:700; cursor:pointer; font-family:inherit; white-space:nowrap; }
  .seg-btn.active { background:var(--accent); color:#fff; }
  .hist-empty { height:100%; display:flex; align-items:center; justify-content:center; color:var(--tm); font-size:13px; }
  .cal-entry { display:flex; align-items:center; gap:8px; }
  .cal-input { background:var(--inset); border:1px solid var(--border); border-radius:9px; padding:8px 10px; font-size:12.5px; color:var(--tp); font-variant-numeric:tabular-nums; }
  .cal-btn { display:inline-flex; align-items:center; gap:7px; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:10px 18px; font-size:13px; font-weight:700; cursor:pointer; }
</style>