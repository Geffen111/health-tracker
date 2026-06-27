<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateLong, formatDateShort } from '$lib/formatDate';
  import Chart from '$lib/Chart.svelte';

  let today = $state(new Date().toISOString().split('T')[0]);
  let selectedDate = $state(today);
  let logs = $state<any[]>([]);
  let loading = $state(true);
  let currentLog = $state<any>(null);
  let darkMode = $state(false);

  onMount(async () => {
    try {
      logs = await invoke('list_daily_logs', { limit: 30, offset: 0 });
    } catch (e) {
      console.error('Error loading sleep data:', e);
    } finally {
      loading = false;
    }
  });

  $effect(() => {
    currentLog = logs.find((l: any) => l.log_date === selectedDate) || null;
  });

  let trendLogs = $derived([...logs].reverse());

  function toggleTheme() {
    darkMode = !darkMode;
    document.documentElement.classList.toggle('dark', darkMode);
  }

  function prevDay() {
    const d = new Date(selectedDate + 'T00:00:00');
    d.setDate(d.getDate() - 1);
    selectedDate = d.toISOString().split('T')[0];
  }

  function nextDay() {
    const d = new Date(selectedDate + 'T00:00:00');
    d.setDate(d.getDate() + 1);
    selectedDate = d.toISOString().split('T')[0];
  }

  let selectedMetric = $state('asleep');

  function pickMetric(k: string) { selectedMetric = k; }

  let metricConfig: Record<string, { label: string; unit: string; field: string; color: string }> = {
    asleep: { label: 'Time asleep', unit: 'h', field: 'sleep_actual_asleep', color: 'var(--accent)' },
    rem: { label: 'REM', unit: 'h', field: 'sleep_rem', color: 'var(--peri)' },
    deep: { label: 'Deep', unit: 'h', field: 'sleep_deep', color: '#3F726A' },
    awake: { label: 'Awake', unit: 'h', field: 'sleep_awake', color: 'var(--amber)' },
    rating: { label: 'Rating', unit: '/10', field: 'sleep_avg', color: 'var(--accent)' },
  };

  let curMetric = $derived(metricConfig[selectedMetric]);
  let curLastVal = $derived(currentLog ? (currentLog[curMetric.field] ?? null) : null);
  let trendValues = $derived(trendLogs.map((l: any) => l[curMetric.field] ?? null).filter((v: number | null): v is number => v != null));
  let curAvgVal = $derived(trendValues.length > 0 ? (trendValues.reduce((a: number, b: number) => a + b, 0) / trendValues.length) : null);
  let curLastFmt = $derived(curMetric.unit === 'h' && curLastVal != null ? curLastVal.toFixed(1) : curLastVal != null ? curLastVal.toFixed(1) : '—');

  function barWidth(val: number | null, field: string): number {
    if (val == null || !currentLog) return 0;
    const total = currentLog.sleep_actual_asleep ?? 0;
    if (total === 0) return 0;
    return Math.max(3, (val / total) * 100);
  }

  let chartLabels = $derived(trendLogs.map((l: any) => formatDateShort(l.log_date)));
  let chartData = $derived(trendLogs.map((l: any) => l[curMetric.field] ?? null));
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
    <button class="theme-btn" onclick={toggleTheme} aria-label="Toggle theme">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/></svg>
    </button>
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
      <div class="section-label">Last night</div>
      <div class="big-hours">{currentLog.sleep_actual_asleep != null ? currentLog.sleep_actual_asleep.toFixed(1) : '—'}<span class="big-unit"> h</span></div>
      <div class="last-night-sub">asleep · rating {currentLog.my_sleep_rating ?? '—'}/10</div>
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
  </div>
{/if}

<div class="trend-card">
  <div class="trend-header">
    <div>
      <div class="card-title">30-day trend</div>
      <div class="card-subtitle">Choose what to plot</div>
    </div>
    <div class="metric-toggle">
      <button class="metric-btn" class:active={selectedMetric === 'asleep'} onclick={() => pickMetric('asleep')}>Asleep</button>
      <button class="metric-btn" class:active={selectedMetric === 'rem'} onclick={() => pickMetric('rem')}>REM</button>
      <button class="metric-btn" class:active={selectedMetric === 'deep'} onclick={() => pickMetric('deep')}>Deep</button>
      <button class="metric-btn" class:active={selectedMetric === 'awake'} onclick={() => pickMetric('awake')}>Awake</button>
      <button class="metric-btn" class:active={selectedMetric === 'rating'} onclick={() => pickMetric('rating')}>Rating</button>
    </div>
  </div>
  <div class="trend-headline">
    <div>
      <div class="trend-metric-label">{curMetric.label} · last night</div>
      <div class="trend-metric-value">{curLastFmt}<span class="trend-unit"> {curMetric.unit}</span></div>
    </div>
    <div class="trend-avg">30-day average <strong>{curAvgVal != null ? (curMetric.unit === '/10' ? curAvgVal.toFixed(1) : curAvgVal.toFixed(1)) : '—'} {curMetric.unit}</strong></div>
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
  .theme-btn { width:36px; height:36px; border-radius:50%; border:1px solid var(--border); background:var(--card); color:var(--ts); display:flex; align-items:center; justify-content:center; cursor:pointer; }
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

  .stat-tiles { display:grid; grid-template-columns:repeat(4,1fr); gap:14px; margin-bottom:16px; }
  .tile { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:16px 18px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:5px; }
  .tile-label { font-size:10px; letter-spacing:.06em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .tile-value { font-family:'Source Serif 4',serif; font-size:24px; font-weight:600; color:var(--tp); }
  .tile-unit { font-size:13px; color:var(--tm); }
  .tile-sub { font-size:11.5px; color:var(--tm); }

  .trend-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:20px 22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:16px; }
  .trend-header { display:flex; justify-content:space-between; align-items:flex-start; gap:14px; flex-wrap:wrap; }
  .card-title { font-family:'Source Serif 4',serif; font-size:18px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; }
  .metric-toggle { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:999px; padding:3px; gap:2px; }
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
