<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateLong, formatDateShort, todayISO, shiftISO } from '$lib/formatDate';
  import { showToast } from '$lib/stores/toast.svelte';
  import Chart from '$lib/Chart.svelte';

  let today = $state(todayISO());
  let selectedDate = $state(today);
  let logs = $state<any[]>([]);
  let loading = $state(true);
  let currentLog = $state<any>(null);

  let rangeDays = $state(30);

  onMount(async () => {
    await loadLogs();
    loading = false;
  });

  async function loadLogs() {
    try {
      logs = await invoke('list_daily_logs', { limit: 60, offset: 0 });
    } catch (e) {
      console.error('Error loading sleep data:', e);
    }
  }

  $effect(() => {
    const date = selectedDate;
    const fromList = logs.find((l: any) => l.log_date === date);
    if (fromList) {
      currentLog = fromList;
      return;
    }
    // Older than the 60 days loaded for the trend — fetch that one day, so a
    // day with data never reads as blank (and never gets typed over by mistake).
    currentLog = null;
    invoke('get_daily_log', { date })
      .then((l: any) => { if (selectedDate === date) currentLog = l ?? null; })
      .catch(() => {});
  });

  // Oldest first, limited to the selected range.
  let trendLogs = $derived([...logs].reverse().slice(-rangeDays));

  // Changing day closes the editor rather than re-pointing a half-typed form at
  // a different night.
  function prevDay() { editing = false; selectedDate = shiftISO(selectedDate, -1); }
  function nextDay() { editing = false; selectedDate = shiftISO(selectedDate, 1); }

  let selectedMetric = $state('score');

  function pickMetric(k: string) { selectedMetric = k; }

  // Sleep Score Avg = mean of my rating + Samsung score (fallback to whichever
  // exists). Historical rows already store this in sleep_avg.
  function sleepScore(l: any): number | null {
    if (!l) return null;
    if (l.sleep_avg != null) return l.sleep_avg;
    const m = l.my_sleep_rating, p = l.phone_sleep_rating;
    if (m != null && p != null) return (m + p) / 2;
    return m ?? p ?? null;
  }

  // The '__score__' sentinel is computed, not a raw column.
  function fieldOf(l: any, field: string): number | null {
    if (!l) return null;
    if (field === '__score__') return sleepScore(l);
    return l[field] ?? null;
  }

  let metricConfig: Record<string, { label: string; unit: string; field: string; color: string }> = {
    score: { label: 'Sleep score', unit: '/10', field: '__score__', color: 'var(--accent)' },
    inbed: { label: 'In bed', unit: 'h', field: 'sleep_time_head_on_pillow', color: 'var(--accent-fg)' },
    asleep: { label: 'Asleep', unit: 'h', field: 'sleep_actual_asleep', color: '#A6CEC4' },
    rem: { label: 'REM', unit: 'h', field: 'sleep_rem', color: 'var(--peri)' },
    deep: { label: 'Deep', unit: 'h', field: 'sleep_deep', color: '#3F726A' },
    awake: { label: 'Awake', unit: 'h', field: 'sleep_awake', color: 'var(--amber)' },
    my: { label: 'My rating', unit: '/10', field: 'my_sleep_rating', color: 'var(--accent)' },
    samsung: { label: 'Samsung', unit: '/10', field: 'phone_sleep_rating', color: 'var(--amber-fg)' },
  };

  // Headline = Sleep Score Avg. Total time in bed = Samsung "head on pillow"
  // (asleep + awake); fall back to the asleep figure for un-synced days.
  let scoreVal = $derived(sleepScore(currentLog));
  let timeInBed = $derived(currentLog ? (currentLog.sleep_time_head_on_pillow ?? currentLog.sleep_actual_asleep ?? null) : null);

  let curMetric = $derived(metricConfig[selectedMetric]);
  let curLastVal = $derived(fieldOf(currentLog, curMetric.field));
  let trendValues = $derived(trendLogs.map((l: any) => fieldOf(l, curMetric.field)).filter((v: number | null): v is number => v != null));
  let curAvgVal = $derived(trendValues.length > 0 ? (trendValues.reduce((a: number, b: number) => a + b, 0) / trendValues.length) : null);
  let curLastFmt = $derived(curLastVal != null ? curLastVal.toFixed(1) : '—');

  function barWidth(val: number | null, field: string): number {
    if (val == null || !currentLog) return 0;
    const total = currentLog.sleep_actual_asleep ?? 0;
    if (total === 0) return 0;
    return Math.max(3, (val / total) * 100);
  }

  let chartLabels = $derived(trendLogs.map((l: any) => formatDateShort(l.log_date)));
  let chartData = $derived(trendLogs.map((l: any) => fieldOf(l, curMetric.field)));

  // ── Manual entry ──────────────────────────────────────────────────────────
  // The stage breakdown normally arrives from the watch CSV import (Settings →
  // sync). Nights the watch wasn't worn, or didn't upload, leave the day blank —
  // this form writes the same five columns by hand for the selected date.

  interface SleepForm {
    inbed: number | null;
    asleep: number | null;
    rem: number | null;
    deep: number | null;
    awake: number | null;
  }

  let isManual = $derived(currentLog?.sleep_source === 'manual');
  let editing = $state(false);
  let saving = $state(false);
  let form = $state<SleepForm>({ inbed: null, asleep: null, rem: null, deep: null, awake: null });

  // <input type="number"> binds to undefined (not null) when emptied.
  function num(v: number | null | undefined): number | null {
    return v == null || Number.isNaN(v) ? null : v;
  }

  function openEditor() {
    form = {
      inbed: currentLog?.sleep_time_head_on_pillow ?? null,
      asleep: currentLog?.sleep_actual_asleep ?? null,
      rem: currentLog?.sleep_rem ?? null,
      deep: currentLog?.sleep_deep ?? null,
      awake: currentLog?.sleep_awake ?? null,
    };
    editing = true;
  }

  // Leaving "in bed" blank mirrors what the sync does: asleep + awake.
  let derivedInBed = $derived(
    num(form.asleep) != null || num(form.awake) != null
      ? (num(form.asleep) ?? 0) + (num(form.awake) ?? 0)
      : null
  );
  let effectiveInBed = $derived(num(form.inbed) ?? derivedInBed);
  let formLight = $derived(
    num(form.asleep) != null ? num(form.asleep)! - (num(form.deep) ?? 0) - (num(form.rem) ?? 0) : null
  );

  // Blocking: values that can't be stored. Warnings: values that are merely odd,
  // which still save — a stage breakdown that doesn't quite add up is better
  // than no record of the night at all.
  let formErrors = $derived.by(() => {
    const errs: string[] = [];
    const fields: [string, number | null][] = [
      ['Time in bed', num(form.inbed)], ['Time asleep', num(form.asleep)],
      ['REM', num(form.rem)], ['Deep', num(form.deep)], ['Awake', num(form.awake)],
    ];
    for (const [label, v] of fields) {
      if (v != null && v < 0) errs.push(`${label} can't be negative.`);
      if (v != null && v > 24) errs.push(`${label} can't be more than 24 hours.`);
    }
    return errs;
  });

  let formWarnings = $derived.by(() => {
    const warns: string[] = [];
    if (formLight != null && formLight < -0.05) warns.push('Deep + REM add up to more than the time asleep.');
    const inBed = effectiveInBed, asleep = num(form.asleep);
    if (inBed != null && asleep != null && asleep > inBed + 0.05) warns.push('Time asleep is longer than time in bed.');
    return warns;
  });

  let formEmpty = $derived(
    num(form.inbed) == null && num(form.asleep) == null && num(form.rem) == null &&
    num(form.deep) == null && num(form.awake) == null
  );

  async function saveBreakdown() {
    if (formErrors.length > 0) return;
    saving = true;
    try {
      await invoke('upsert_sleep_breakdown', {
        breakdown: {
          log_date: selectedDate,
          sleep_time_head_on_pillow: effectiveInBed,
          sleep_actual_asleep: num(form.asleep),
          sleep_rem: num(form.rem),
          sleep_deep: num(form.deep),
          sleep_awake: num(form.awake),
        },
      });
      await loadLogs();
      editing = false;
      showToast(formEmpty ? 'Cleared — the watch sync owns this night again' : 'Sleep breakdown saved');
    } catch (e) {
      showToast(`Couldn't save sleep: ${e}`, 'error');
    } finally {
      saving = false;
    }
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Sleep</div>
    <div class="page-subtitle">Last night's stages and how sleep is trending</div>
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
    <button class="manual-btn" onclick={openEditor} disabled={editing}>
      {currentLog ? 'Edit sleep' : 'Enter manually'}
    </button>
  </div>
</div>

{#if editing}
  <div class="card editor-card">
    <div class="card-heading-row">
      <div>
        <div class="card-title">Sleep breakdown · {formatDateLong(selectedDate)}</div>
        <div class="card-subtitle">For nights the watch sync missed or got wrong. Leave a field blank to clear it. Sleep ratings live on the Daily Log page.</div>
      </div>
      {#if isManual}
        <span class="manual-badge">Manual</span>
      {/if}
    </div>

    <div class="field-grid">
      <div class="text-field">
        <label for="f-asleep">Time asleep</label>
        <div class="input-unit">
          <input id="f-asleep" type="number" min="0" max="24" step="0.1" bind:value={form.asleep} placeholder="—" />
          <span class="unit-label">h</span>
        </div>
      </div>
      <div class="text-field">
        <label for="f-awake">Awake</label>
        <div class="input-unit">
          <input id="f-awake" type="number" min="0" max="24" step="0.1" bind:value={form.awake} placeholder="—" />
          <span class="unit-label">h</span>
        </div>
      </div>
      <div class="text-field">
        <label for="f-inbed">Time in bed <span class="label-hint">· optional</span></label>
        <div class="input-unit">
          <input id="f-inbed" type="number" min="0" max="24" step="0.1" bind:value={form.inbed} placeholder={derivedInBed != null ? derivedInBed.toFixed(1) : '—'} />
          <span class="unit-label">h</span>
        </div>
        <div class="field-hint">Blank = asleep + awake{derivedInBed != null ? ` (${derivedInBed.toFixed(1)}h)` : ''}</div>
      </div>
      <div class="text-field">
        <label for="f-deep">Deep</label>
        <div class="input-unit">
          <input id="f-deep" type="number" min="0" max="24" step="0.1" bind:value={form.deep} placeholder="—" />
          <span class="unit-label">h</span>
        </div>
      </div>
      <div class="text-field">
        <label for="f-rem">REM</label>
        <div class="input-unit">
          <input id="f-rem" type="number" min="0" max="24" step="0.1" bind:value={form.rem} placeholder="—" />
          <span class="unit-label">h</span>
        </div>
      </div>
      <div class="text-field">
        <span class="pseudo-label">Light <span class="label-hint">· calculated</span></span>
        <div class="computed-box">{formLight != null ? formLight.toFixed(1) : '—'}<span class="unit-label"> h</span></div>
        <div class="field-hint">Asleep − deep − REM</div>
      </div>
    </div>

    {#each formErrors as err}
      <div class="form-msg error">{err}</div>
    {/each}
    {#each formWarnings as warn}
      <div class="form-msg warn">{warn}</div>
    {/each}

    <div class="editor-footer">
      <span class="editor-note">
        {#if formEmpty}
          Saving with every field blank hands this night back to the watch sync.
        {:else}
          Saved values stick — a later watch sync won't overwrite them.
        {/if}
      </span>
      <div class="editor-actions">
        <button class="cancel-btn" onclick={() => (editing = false)} disabled={saving}>Cancel</button>
        <button class="save-btn" onclick={saveBreakdown} disabled={saving || formErrors.length > 0}>
          {saving ? 'Saving…' : 'Save breakdown'}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if loading}
  <p class="loading-text">Loading...</p>
{:else if !currentLog}
  <div class="empty-card">
    <p>No sleep data recorded for this date.</p>
    {#if !editing}
      <button class="save-btn" onclick={openEditor}>Enter it manually</button>
    {/if}
  </div>
{:else}
  <div class="last-night-card">
    <div class="last-night-left">
      <div class="section-label">
        Last night · sleep score
        {#if isManual}<span class="manual-badge" title="Typed in by hand — the watch sync won't overwrite it">Manual</span>{/if}
      </div>
      <div class="big-hours">{scoreVal != null ? scoreVal.toFixed(1) : '—'}<span class="big-unit"> /10</span></div>
      <div class="last-night-sub">{timeInBed != null ? timeInBed.toFixed(1) : '—'}h in bed · {currentLog.sleep_actual_asleep != null ? currentLog.sleep_actual_asleep.toFixed(1) : '—'}h asleep</div>
    </div>
    <div class="last-night-right">
      <div class="stage-bar">
        <div class="stage-seg" style="width:{barWidth(currentLog.sleep_deep, 'sleep_deep')}%;background:#3F726A;"></div>
        <div class="stage-seg" style="width:{barWidth(currentLog.sleep_rem, 'sleep_rem')}%;background:var(--peri);"></div>
        <div class="stage-seg" style="width:{barWidth(currentLog.sleep_actual_asleep, 'sleep_actual_asleep') - barWidth(currentLog.sleep_deep, 'sleep_deep') - barWidth(currentLog.sleep_rem, 'sleep_rem')}%;background:#A6CEC4;"></div>
        <div class="stage-seg" style="width:{barWidth(currentLog.sleep_awake, 'sleep_awake')}%;background:var(--amber);"></div>
      </div>
      <div class="stage-legend">
        <span><span class="legend-swatch" style="background:#3F726A;"></span>Deep {currentLog.sleep_deep?.toFixed(1) ?? '0'}h</span>
        <span><span class="legend-swatch" style="background:var(--peri);"></span>REM {currentLog.sleep_rem?.toFixed(1) ?? '0'}h</span>
        <span><span class="legend-swatch" style="background:#A6CEC4;"></span>Light {(currentLog.sleep_actual_asleep != null && currentLog.sleep_deep != null && currentLog.sleep_rem != null ? (currentLog.sleep_actual_asleep - currentLog.sleep_deep - currentLog.sleep_rem).toFixed(1) : '0')}h</span>
        <span><span class="legend-swatch" style="background:var(--amber);"></span>Awake {currentLog.sleep_awake?.toFixed(1) ?? '0'}h</span>
      </div>
    </div>
  </div>

  <div class="stat-tiles">
    <div class="tile">
      <div class="tile-label">Time asleep</div>
      <div class="tile-value">{currentLog.sleep_actual_asleep != null ? currentLog.sleep_actual_asleep.toFixed(1) : '—'}<span class="tile-unit"> h</span></div>
      <div class="tile-sub">avg {trendValues.length > 0 ? (trendValues.reduce((a: number, b: number) => a + b, 0) / trendValues.length).toFixed(1) : '—'}h</div>
    </div>
    <div class="tile">
      <div class="tile-label">REM</div>
      <div class="tile-value">{currentLog.sleep_rem != null ? currentLog.sleep_rem.toFixed(1) : '—'}<span class="tile-unit"> h</span></div>
      <div class="tile-sub">{currentLog.sleep_actual_asleep ? Math.round((currentLog.sleep_rem ?? 0) / currentLog.sleep_actual_asleep * 100) : 0}% of sleep</div>
    </div>
    <div class="tile">
      <div class="tile-label">Deep</div>
      <div class="tile-value">{currentLog.sleep_deep != null ? currentLog.sleep_deep.toFixed(1) : '—'}<span class="tile-unit"> h</span></div>
      <div class="tile-sub">{currentLog.sleep_actual_asleep ? Math.round((currentLog.sleep_deep ?? 0) / currentLog.sleep_actual_asleep * 100) : 0}% of sleep</div>
    </div>
    <div class="tile">
      <div class="tile-label">Awake</div>
      <div class="tile-value">{currentLog.sleep_awake != null ? currentLog.sleep_awake.toFixed(1) : '—'}<span class="tile-unit"> h</span></div>
      <div class="tile-sub">{currentLog.sleep_awake != null && currentLog.sleep_awake > 0 ? 'Brief wakes' : 'None'}</div>
    </div>
    <div class="tile">
      <div class="tile-label">My score</div>
      <div class="tile-value">{currentLog.my_sleep_rating != null ? currentLog.my_sleep_rating.toFixed(1) : '—'}<span class="tile-unit"> /10</span></div>
      <div class="tile-sub">from daily log</div>
    </div>
    <div class="tile">
      <div class="tile-label">Samsung score</div>
      <div class="tile-value">{currentLog.phone_sleep_rating != null ? currentLog.phone_sleep_rating.toFixed(1) : '—'}<span class="tile-unit"> /10</span></div>
      <div class="tile-sub">{currentLog.phone_sleep_rating != null ? 'from daily log' : 'not entered'}</div>
    </div>
  </div>
{/if}

<div class="trend-card">
  <div class="trend-header">
    <div>
      <div class="card-title">{rangeDays}-day trend</div>
      <div class="card-subtitle">Choose what to plot</div>
      <div class="seg-range">
        <button class="metric-btn" class:active={rangeDays === 14} onclick={() => rangeDays = 14}>14D</button>
        <button class="metric-btn" class:active={rangeDays === 30} onclick={() => rangeDays = 30}>30D</button>
        <button class="metric-btn" class:active={rangeDays === 60} onclick={() => rangeDays = 60}>60D</button>
      </div>
    </div>
    <div class="metric-toggle">
      <button class="metric-btn" class:active={selectedMetric === 'score'} onclick={() => pickMetric('score')}>Score</button>
      <button class="metric-btn" class:active={selectedMetric === 'inbed'} onclick={() => pickMetric('inbed')}>In bed</button>
      <button class="metric-btn" class:active={selectedMetric === 'asleep'} onclick={() => pickMetric('asleep')}>Asleep</button>
      <button class="metric-btn" class:active={selectedMetric === 'rem'} onclick={() => pickMetric('rem')}>REM</button>
      <button class="metric-btn" class:active={selectedMetric === 'deep'} onclick={() => pickMetric('deep')}>Deep</button>
      <button class="metric-btn" class:active={selectedMetric === 'awake'} onclick={() => pickMetric('awake')}>Awake</button>
      <button class="metric-btn" class:active={selectedMetric === 'my'} onclick={() => pickMetric('my')}>My rating</button>
      <button class="metric-btn" class:active={selectedMetric === 'samsung'} onclick={() => pickMetric('samsung')}>Samsung</button>
    </div>
  </div>
  <div class="trend-headline">
    <div>
      <div class="trend-metric-label">{curMetric.label} · last night</div>
      <div class="trend-metric-value">{curLastFmt}<span class="trend-unit"> {curMetric.unit}</span></div>
    </div>
    <div class="trend-avg">{rangeDays}-day average <strong>{curAvgVal != null ? curAvgVal.toFixed(1) : '—'} {curMetric.unit}</strong></div>
  </div>
  <div style="height:200px;">
    <Chart
      type="line"
      labels={chartLabels}
      datasets={[
        {
          label: curMetric.label,
          data: chartData.filter((v): v is number => v != null),
          borderColor: curMetric.color,
          backgroundColor: curMetric.color,
        },
      ]}
      options={{
        elements: { point: { radius: 2, hoverRadius: 5 } },
        scales: {
          y: { beginAtZero: true, grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } } },
          x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 }, maxTicksLimit: 6 } },
        },
        plugins: { legend: { display: false } },
      }}
      chartArea="200px"
    />
  </div>
  <div class="chart-xlabels">
    {#each trendLogs as log, i}
      {#if i === 0 || i === Math.floor(trendLogs.length / 4) || i === Math.floor(trendLogs.length / 2) || i === Math.floor(3 * trendLogs.length / 4) || i === trendLogs.length - 1}
        <span>{formatDateShort(log.log_date)}</span>
      {/if}
    {/each}
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
  .loading-text { color:var(--ts); padding:32px; text-align:center; }
  .empty-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:32px; box-shadow:var(--shadow); text-align:center; color:var(--ts); display:flex; flex-direction:column; align-items:center; gap:14px; }
  .manual-btn { background:var(--card); border:1px solid var(--border); color:var(--ts); border-radius:999px; padding:9px 14px; font-size:12.5px; font-weight:600; cursor:pointer; white-space:nowrap; }
  .manual-btn:disabled { color:var(--tm); cursor:default; }
  .manual-badge { display:inline-block; margin-left:8px; font-size:9.5px; letter-spacing:.06em; font-weight:800; color:var(--accent-fg); background:var(--accent-soft); border-radius:999px; padding:2px 8px; vertical-align:middle; }

  .card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:18px; }
  .editor-card { margin-bottom:16px; }
  .card-heading-row { display:flex; align-items:flex-start; justify-content:space-between; gap:12px; }
  .field-grid { display:grid; grid-template-columns:repeat(3,1fr); gap:16px; }
  .text-field { display:flex; flex-direction:column; gap:8px; }
  .text-field label, .pseudo-label { font-size:13.5px; font-weight:600; color:var(--tp); }
  .label-hint { font-weight:500; color:var(--tm); font-size:11.5px; }
  .field-hint { font-size:11px; color:var(--tm); }
  .input-unit { display:flex; align-items:center; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:4px 6px; }
  .input-unit input { width:100%; background:transparent; border:none; padding:7px; font-size:13.5px; color:var(--tp); font-variant-numeric:tabular-nums; }
  .unit-label { font-size:12px; color:var(--tm); padding-right:8px; white-space:nowrap; }
  .computed-box { display:flex; align-items:center; border:1px dashed var(--border); border-radius:12px; padding:11px 13px; font-size:13.5px; color:var(--ts); font-variant-numeric:tabular-nums; }

  .form-msg { font-size:12.5px; border-radius:12px; padding:9px 13px; }
  .form-msg.error { color:var(--amber-fg); background:var(--inset); border:1px solid var(--amber); }
  .form-msg.warn { color:var(--ts); background:var(--inset); border:1px solid var(--border); }

  .editor-footer { display:flex; align-items:center; justify-content:space-between; gap:12px; flex-wrap:wrap; }
  .editor-note { font-size:11.5px; color:var(--tm); }
  .editor-actions { display:flex; align-items:center; gap:10px; }
  .cancel-btn { background:transparent; border:1px solid var(--border); color:var(--ts); border-radius:999px; padding:10px 18px; font-size:13px; font-weight:600; cursor:pointer; }
  .save-btn { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:11px 22px; font-size:13.5px; font-weight:700; cursor:pointer; }
  .save-btn:disabled { opacity:.55; cursor:not-allowed; }

  .last-night-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; gap:28px; align-items:center; margin-bottom:16px; flex-wrap:wrap; }
  .last-night-left { display:flex; flex-direction:column; gap:3px; min-width:120px; }
  .section-label { font-size:10.5px; letter-spacing:.07em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .big-hours { font-family:'Source Serif 4',serif; font-size:38px; font-weight:600; color:var(--tp); letter-spacing:-.02em; line-height:1; }
  .big-unit { font-size:18px; color:var(--tm); }
  .last-night-sub { font-size:12.5px; color:var(--ts); }
  .last-night-right { flex:1; min-width:280px; display:flex; flex-direction:column; gap:10px; }

  .stage-bar { display:flex; height:22px; border-radius:8px; overflow:hidden; border:1px solid var(--border); }
  .stage-seg { min-width:4px; }
  .stage-legend { display:flex; gap:18px; flex-wrap:wrap; font-size:12px; color:var(--ts); }
  .legend-swatch { display:inline-block; width:10px; height:10px; border-radius:3px; vertical-align:middle; margin-right:6px; }

  .stat-tiles { display:grid; grid-template-columns:repeat(3,1fr); gap:14px; margin-bottom:16px; }
  .tile { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:16px 18px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:5px; }
  .tile-label { font-size:10px; letter-spacing:.06em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .tile-value { font-family:'Source Serif 4',serif; font-size:24px; font-weight:600; color:var(--tp); }
  .tile-unit { font-size:13px; color:var(--tm); }
  .tile-sub { font-size:11.5px; color:var(--tm); }

  .trend-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:20px 22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:16px; }
  .trend-header { display:flex; justify-content:space-between; align-items:flex-start; gap:14px; flex-wrap:wrap; }
  .card-title { font-family:'Source Serif 4',serif; font-size:18px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; }
  .seg-range { display:inline-flex; margin-top:10px; background:var(--inset); border:1px solid var(--border); border-radius:999px; padding:3px; gap:2px; }
  .metric-toggle { display:flex; flex-wrap:wrap; background:var(--inset); border:1px solid var(--border); border-radius:16px; padding:3px; gap:2px; }
  .metric-btn { background:transparent; border:none; border-radius:999px; padding:6px 14px; font-size:12.5px; font-weight:700; cursor:pointer; white-space:nowrap; color:var(--ts); font-family:inherit; }
  .metric-btn.active { background:var(--accent); color:#fff; }

  .trend-headline { display:flex; align-items:flex-end; gap:18px; }
  .trend-metric-label { font-size:10.5px; letter-spacing:.06em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .trend-metric-value { font-family:'Source Serif 4',serif; font-size:28px; font-weight:600; color:var(--tp); }
  .trend-unit { font-size:14px; color:var(--tm); }
  .trend-avg { font-size:12.5px; color:var(--ts); padding-bottom:6px; }
  .trend-avg strong { color:var(--tp); font-variant-numeric:tabular-nums; }

  .chart-xlabels { display:flex; justify-content:space-between; font-size:10.5px; color:var(--tm); font-weight:700; padding:0 4px; }
</style>
