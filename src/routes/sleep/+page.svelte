<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateLong, formatDateShort, todayISO, shiftISO } from '$lib/formatDate';
  import Chart from '$lib/Chart.svelte';

  let today = $state(todayISO());
  let selectedDate = $state(today);
  let logs = $state<any[]>([]);
  let loading = $state(true);
  let currentLog = $state<any>(null);

  let rangeDays = $state(30);

  onMount(async () => {
    try {
      logs = await invoke('list_daily_logs', { limit: 60, offset: 0 });
    } catch (e) {
      console.error('Error loading sleep data:', e);
    } finally {
      loading = false;
    }
  });

  $effect(() => {
    currentLog = logs.find((l: any) => l.log_date === selectedDate) || null;
  });

  // Oldest first, limited to the selected range.
  let trendLogs = $derived([...logs].reverse().slice(-rangeDays));

  function prevDay() { selectedDate = shiftISO(selectedDate, -1); }
  function nextDay() { selectedDate = shiftISO(selectedDate, 1); }

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
  </div>
</div>

{#if loading}
  <p class="loading-text">Loading...</p>
{:else if !currentLog}
  <div class="empty-card">
    <p>No sleep data recorded for this date.</p>
  </div>
{:else}
  <div class="last-night-card">
    <div class="last-night-left">
      <div class="section-label">Last night · sleep score</div>
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
  .empty-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:32px; box-shadow:var(--shadow); text-align:center; color:var(--ts); }

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
