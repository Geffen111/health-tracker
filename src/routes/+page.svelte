<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate, todayISO, shiftISO } from '$lib/formatDate';
  import Chart from '$lib/Chart.svelte';

  let summary = $state<any>(null);
  let todayLog = $state<any>(null);
  let predictions = $state<any[]>([]);
  let logs = $state<any[]>([]);
  let loading = $state(true);
  let rangeDays = $state(14);
  let metricA = $state<string | null>('fatigue');
  let metricB = $state<string | null>('steps');

  const METRICS: Record<string, { label: string; field: string; color: string; format: (v: number) => string }> = {
    fatigue: { label: 'Fatigue', field: 'fatigue_rating', color: 'var(--accent)', format: (v) => v.toFixed(1) },
    sleep: { label: 'Sleep score', field: 'sleep_avg', color: 'var(--teal)', format: (v) => v.toFixed(1) },
    steps: { label: 'Steps', field: 'steps', color: 'var(--peri)', format: (v) => Math.round(v).toLocaleString() },
    restingHr: { label: 'Resting HR', field: 'ave_resting_hr', color: 'var(--amber)', format: (v) => v.toFixed(0) },
    headache: { label: 'Headache', field: 'headache_rating', color: 'var(--red)', format: (v) => v.toFixed(1) },
    pemRisk: { label: 'PEM Risk', field: 'predicted_pem_risk', color: 'var(--purple)', format: (v) => v.toFixed(1) },
  };

  onMount(async () => {
    try {
      const [s, log, preds] = await Promise.all([
        invoke<any>('get_dashboard_summary'),
        invoke<any>('get_daily_log', { date: todayISO() }),
        invoke<any[]>('get_pem_predictions', { limit: 60 }),
      ]);
      summary = s;
      todayLog = log;
      predictions = preds;
      logs = await invoke<any[]>('list_daily_logs', { limit: 60, offset: 0 });
    } catch (e) {
      console.error('Dashboard error:', e);
    } finally {
      loading = false;
    }
  });

  // Sleep Score Avg = mean of my rating + Samsung score (fallback to whichever
  // exists). Historical rows already store this in sleep_avg.
  function sleepScore(log: any): number | null {
    if (!log) return null;
    if (log.sleep_avg != null) return log.sleep_avg;
    const m = log.my_sleep_rating, p = log.phone_sleep_rating;
    if (m != null && p != null) return (m + p) / 2;
    return m ?? p ?? null;
  }

  // Steps & cardio are full-day synced metrics — today's are incomplete, so the
  // dashboard shows yesterday's complete totals for those. Sleep is "last night"
  // (complete by morning) so it stays on today's row.
  let yesterdayLog = $derived(logs.find((l: any) => l.log_date === shiftISO(todayISO(), -1)) ?? null);

  let fatigue = $derived(todayLog?.fatigue_rating ?? null);
  let sleep = $derived(sleepScore(todayLog));
  let steps = $derived(yesterdayLog?.steps ?? null);
  let restingHr = $derived(yesterdayLog?.ave_resting_hr ?? null);
  // Today's crash risk is the PEM prediction computed from yesterday's load.
  let yesterdayPred = $derived(predictions?.find((p: any) => p.log_date === shiftISO(todayISO(), -1)) ?? null);
  let riskBand = $derived(yesterdayPred?.risk_band ?? summary?.current_risk_band ?? null);
  let riskScore = $derived(yesterdayPred?.predicted_next_day_fatigue ?? null);

  function gaugeArc(score: number | null): { pct: number; color: string } {
    if (score == null) return { pct: 0, color: 'var(--inset)' };
    const pct = Math.min(100, (score / 10) * 100);
    const color = score >= 4.5 ? 'var(--red)' : score >= 2 ? 'var(--amber)' : 'var(--accent)';
    return { pct, color };
  }

  let gauge = $derived(gaugeArc(riskScore));

  function bandColor(band: string | null): string {
    if (band === 'High') return 'var(--red-fg)';
    if (band === 'Medium') return 'var(--amber-fg)';
    return 'var(--accent-fg)';
  }

  function bandBg(band: string | null): string {
    if (band === 'High') return 'var(--red-soft)';
    if (band === 'Medium') return 'var(--amber-soft)';
    return 'var(--accent-soft)';
  }

  let gaugeArcPath = $derived.by(() => {
    if (riskScore == null) return '';
    const r = 54, cx = 70, cy = 78;
    const startAngle = Math.PI * 0.75;
    const endAngle = Math.PI * 2.25;
    const pct = Math.min(1, riskScore / 10);
    const angle = startAngle + pct * (endAngle - startAngle);
    const sx = cx + r * Math.cos(startAngle);
    const sy = cy + r * Math.sin(startAngle);
    const ex = cx + r * Math.cos(angle);
    const ey = cy + r * Math.sin(angle);
    const large = pct > 0.5 ? 1 : 0;
    return `M${sx} ${sy} A${r} ${r} 0 ${large} 1 ${ex} ${ey}`;
  });

  let todayStr = $derived(formatDate(todayISO()));

  function bandLabel(band: string | null): string {
    if (band === 'High') return 'High — rest today';
    if (band === 'Medium') return 'Medium — pace gently';
    return 'Low — good to go';
  }

  function fieldVal(log: any, field: string): number | null {
    if (field === 'predicted_pem_risk') {
      const p = predictions.find((x: any) => x.log_date === log.log_date);
      return p?.predicted_pem_risk ?? null;
    }
    if (field === 'sleep_avg') return sleepScore(log);
    return log[field] ?? null;
  }

  // Clicking a selected signal turns it off; otherwise it fills the first free
  // slot (A = left axis, B = right axis), replacing the secondary if both full.
  function toggleMetric(key: string) {
    if (metricA === key) { metricA = null; return; }
    if (metricB === key) { metricB = null; return; }
    if (metricA == null) { metricA = key; return; }
    if (metricB == null) { metricB = key; return; }
    metricB = key;
  }

  let chartLogs = $derived([...logs].reverse().slice(-rangeDays));
  let chartLabels = $derived(chartLogs.map((l: any) => formatDate(l.log_date)));
  let chartMetricA = $derived(metricA ? METRICS[metricA] : null);
  let chartMetricB = $derived(metricB ? METRICS[metricB] : null);

  let compareDatasets = $derived([
    ...(chartMetricA ? [{
      label: chartMetricA.label,
      data: chartLogs.map((l: any) => fieldVal(l, chartMetricA!.field)),
      borderColor: chartMetricA.color,
      backgroundColor: chartMetricA.color,
      yAxisID: 'y',
    }] : []),
    ...(chartMetricB ? [{
      label: chartMetricB.label,
      data: chartLogs.map((l: any) => fieldVal(l, chartMetricB!.field)),
      borderColor: chartMetricB.color,
      backgroundColor: chartMetricB.color,
      yAxisID: 'y1',
    }] : []),
  ]);

  let compareOptions = $derived({
    elements: { point: { radius: 2, hoverRadius: 5 } },
    spanGaps: true,
    interaction: { mode: 'index', intersect: false },
    scales: {
      y: { type: 'linear', position: 'left', beginAtZero: true, grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } } },
      ...(chartMetricB ? { y1: { type: 'linear', position: 'right', beginAtZero: true, grid: { drawOnChartArea: false }, ticks: { color: 'var(--ts)', font: { size: 11 } } } } : {}),
      x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 }, maxTicksLimit: 6 } },
    },
    plugins: { legend: { display: true, labels: { color: 'var(--ts)', font: { size: 11 }, boxWidth: 10, padding: 12 } } },
  });

  let sleepLogs = $derived([...logs].reverse().slice(-14));
  let sleepChartData = $derived(sleepLogs.map((l: any) => sleepScore(l)));
</script>

<div class="page-header">
  <div>
    <div class="page-title">Good morning</div>
    <div class="page-subtitle">Today · {todayStr} — here's how you're tracking</div>
  </div>
  <div class="header-actions">
    <a href="/daily" class="primary-btn">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      Log today
    </a>
  </div>
</div>

{#if loading}
  <p class="loading-text">Loading...</p>
{:else if summary}
  <div class="top-row">
    <div class="risk-card">
      <div class="risk-gauge">
        <svg viewBox="0 0 140 92" width="140" height="92">
          <path d="M16 78 A54 54 0 0 1 124 78" fill="none" stroke="var(--inset)" stroke-width="11" stroke-linecap="round"/>
          {#if gaugeArcPath}
            <path d={gaugeArcPath} fill="none" stroke={gauge.color} stroke-width="11" stroke-linecap="round"/>
          {/if}
        </svg>
        <div class="gauge-value">{riskScore != null ? riskScore.toFixed(1) : '—'}</div>
        <div class="gauge-of">of 10</div>
      </div>
      <div class="risk-info">
        <div class="risk-header">
          <div class="risk-label">Predicted fatigue</div>
          <span class="risk-band" style="color:{bandColor(riskBand)};background:{bandBg(riskBand)};">
            {riskBand ?? '—'}
          </span>
        </div>
        <div class="risk-desc">{bandLabel(riskBand)}</div>
        <div class="risk-tags">
          {#if fatigue != null}
            <span class="risk-tag">↑ Fatigue {fatigue}/10</span>
          {/if}
          {#if sleep != null && sleep < 7}
            <span class="risk-tag">↓ Sleep {sleep.toFixed(1)}/10</span>
          {/if}
        </div>
      </div>
    </div>

    <div class="mini-card-group">
      <div class="mini-card">
        <div class="mini-inset">
          <div class="mini-label">Fatigue</div>
          <div class="mini-value">{fatigue != null ? fatigue.toFixed(1) : '—'}<span class="mini-unit"> /10</span></div>
        </div>
        <div class="mini-inset">
          <div class="mini-label">{todayLog?.phone_sleep_rating != null ? 'Sleep score' : 'My sleep score'}</div>
          <div class="mini-value">{sleep != null ? sleep.toFixed(1) : '—'}<span class="mini-unit"> /10</span></div>
        </div>
      </div>
      <div class="mini-card">
        <div class="mini-inset">
          <div class="mini-label">Steps · yday</div>
          <div class="mini-value">{steps != null ? Number(steps).toLocaleString() : '—'}</div>
        </div>
        <div class="mini-inset">
          <div class="mini-label">Resting HR</div>
          <div class="mini-value">{restingHr ?? '—'}<span class="mini-unit"> bpm</span></div>
        </div>
      </div>
    </div>
  </div>

    <div class="compare-card">
    <div class="compare-header">
      <div>
        <div class="card-title">Compare signals</div>
        <div class="card-subtitle">See how any two measures move together</div>
      </div>
      <div class="range-toggle">
        <button class="range-btn" class:active={rangeDays === 14} onclick={() => rangeDays = 14}>14D</button>
        <button class="range-btn" class:active={rangeDays === 30} onclick={() => rangeDays = 30}>30D</button>
        <button class="range-btn" class:active={rangeDays === 60} onclick={() => rangeDays = 60}>60D</button>
      </div>
    </div>
    <div class="metric-picker-row">
      {#each Object.entries(METRICS) as [key, m]}
        <button class="metric-pill" class:accent={metricA === key} class:peri={metricB === key} onclick={() => toggleMetric(key)}>
          <span class="pill-dot" style="background:{m.color};"></span>
          {m.label}
          {#if metricA === key}<span class="pill-axis">L</span>{:else if metricB === key}<span class="pill-axis">R</span>{/if}
        </button>
      {/each}
    </div>
    <div style="height:200px;">
      {#if compareDatasets.length === 0}
        <div class="compare-empty">Pick a signal above to plot.</div>
      {:else}
        <Chart
          type="line"
          labels={chartLabels}
          datasets={compareDatasets}
          options={compareOptions}
          chartArea="200px"
        />
      {/if}
    </div>
  </div>

  <div class="bottom-row">
    <div class="stat-card">
      <div class="stat-label">Recovery debt</div>
      <div class="stat-row">
        <span class="stat-value">{(yesterdayPred?.predicted_pem_risk ?? 0).toFixed(1)}</span>
        <span class="stat-threshold">/ 4.0 threshold</span>
      </div>
      <div class="progress-bar">
        <div class="progress-fill" style="width:45%;background:var(--accent);"></div>
      </div>
      <div class="stat-desc">Comfortably below crash line</div>
    </div>
    <div class="stat-card">
      <div class="stat-card-header">
        <span class="stat-label">Sleep score · 14 nights</span>
        <span class="stat-avg">avg {summary.sleep_last_30d?.toFixed(1) ?? '—'}/10</span>
      </div>
      <div style="height:50px;">
        <Chart
          type="line"
          labels={sleepLogs.map((l: any) => '')}
          datasets={[{ label: 'Sleep', data: sleepChartData, borderColor: 'var(--accent)', backgroundColor: 'var(--accent)' }]}
          options={{
            elements: { point: { radius: 0 }, line: { tension: 0.3 } },
            scales: { x: { display: false }, y: { display: false, beginAtZero: true } },
            plugins: { legend: { display: false }, tooltip: { enabled: false } },
          }}
          chartArea="50px"
        />
      </div>
      <div class="stat-desc">Fairly steady this fortnight</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Risk · last 7 days</div>
      <div class="risk-dots">
        <span class="risk-dot low"></span>
        <span class="risk-dot low"></span>
        <span class="risk-dot med"></span>
        <span class="risk-dot low"></span>
        <span class="risk-dot low"></span>
        <span class="risk-dot med"></span>
        <span class="risk-dot med"></span>
      </div>
      <div class="stat-desc">{summary.crash_count_30d > 0 ? `${summary.crash_count_30d} crashes in 30 days` : 'No crashes in 30 days'}</div>
    </div>
  </div>
{:else}
  <p class="empty-text">No data yet. Start by importing your spreadsheet or adding a daily log entry.</p>
{/if}

<style>
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 24px;
  }
  .page-title {
    font-family: 'Source Serif 4', serif;
    font-size: 30px;
    font-weight: 600;
    color: var(--tp);
    letter-spacing: -0.01em;
  }
  .page-subtitle {
    font-size: 13.5px;
    color: var(--ts);
    margin-top: 3px;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .primary-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 999px;
    padding: 10px 16px;
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
    text-decoration: none;
    white-space: nowrap;
  }
  .loading-text, .empty-text {
    color: var(--ts);
    font-size: 14px;
    text-align: center;
    padding: 48px;
  }

  .top-row {
    display: flex;
    gap: 14px;
    margin-bottom: 16px;
  }

  .risk-card {
    flex: 1.7;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 18px;
    box-shadow: var(--shadow-lg);
    display: flex;
    gap: 18px;
    align-items: center;
  }
  .risk-gauge {
    flex-shrink: 0;
    width: 140px;
    height: 92px;
    position: relative;
  }
  .gauge-value {
    position: absolute;
    left: 0;
    right: 0;
    top: 40px;
    text-align: center;
    font-family: 'Source Serif 4', serif;
    font-size: 34px;
    font-weight: 600;
    color: var(--tp);
    letter-spacing: -0.02em;
  }
  .gauge-of {
    position: absolute;
    left: 0;
    right: 0;
    top: 80px;
    text-align: center;
    font-size: 10px;
    color: var(--tm);
    font-weight: 600;
  }
  .risk-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .risk-header {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .risk-label {
    font-size: 10.5px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    font-weight: 800;
    color: var(--ts);
  }
  .risk-band {
    font-size: 11px;
    font-weight: 800;
    padding: 3px 10px;
    border-radius: 999px;
  }
  .risk-desc {
    font-size: 13.5px;
    color: var(--ts);
    line-height: 1.45;
  }
  .risk-tags {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 2px;
  }
  .risk-tag {
    font-size: 11.5px;
    color: var(--ts);
    background: var(--inset);
    border: 1px solid var(--border);
    padding: 4px 9px;
    border-radius: 999px;
  }

  .mini-card-group {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .mini-card {
    flex: 1;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 16px;
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .mini-inset {
    background: var(--inset);
    border-radius: 12px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .mini-label {
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    font-weight: 800;
    color: var(--ts);
  }
  .mini-value {
    font-family: 'Source Serif 4', serif;
    font-size: 23px;
    font-weight: 600;
    color: var(--tp);
  }
  .mini-unit {
    font-size: 13px;
    color: var(--tm);
  }

  .compare-card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 18px 20px;
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 15px;
    margin-bottom: 16px;
  }
  .compare-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
  }
  .card-title {
    font-family: 'Source Serif 4', serif;
    font-size: 18px;
    font-weight: 600;
    color: var(--tp);
  }
  .card-subtitle {
    font-size: 12.5px;
    color: var(--ts);
    margin-top: 2px;
  }
  .range-toggle {
    display: flex;
    background: var(--inset);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 3px;
    gap: 2px;
  }
  .range-btn {
    background: transparent;
    border: none;
    color: var(--ts);
    border-radius: 999px;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
    font-family: inherit;
  }
  .range-btn.active {
    background: var(--accent);
    color: #fff;
  }
  .metric-picker-row {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .metric-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    font-weight: 700;
    border: 1px solid var(--border);
    padding: 6px 12px;
    border-radius: 999px;
    cursor: pointer;
  }
  .metric-pill.accent {
    color: var(--accent-fg);
    background: var(--accent-soft);
  }
  .metric-pill.peri {
    color: var(--peri);
    background: var(--peri-soft);
  }
  .pill-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .metric-pill.accent .pill-dot { background: var(--accent); }
.metric-pill.peri .pill-dot { background: var(--peri); }
  .pill-axis {
    font-size: 9.5px;
    font-weight: 800;
    line-height: 1;
    padding: 2px 4px;
    border-radius: 4px;
    background: var(--card);
    border: 1px solid currentColor;
  }
  .compare-empty {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--tm);
    font-size: 13px;
  }

  .bottom-row {
    display: flex;
    gap: 14px;
  }
  .stat-card {
    flex: 1;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 16px 18px;
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .stat-label {
    font-size: 10.5px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    font-weight: 800;
    color: var(--ts);
  }
  .stat-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .stat-value {
    font-family: 'Source Serif 4', serif;
    font-size: 26px;
    font-weight: 600;
    color: var(--tp);
  }
  .stat-threshold {
    font-size: 12px;
    color: var(--tm);
  }
  .stat-avg {
    font-size: 11.5px;
    color: var(--tm);
    font-weight: 700;
  }
  .stat-card-header {
    display: flex;
    justify-content: space-between;
  }
  .progress-bar {
    height: 8px;
    border-radius: 999px;
    background: var(--inset);
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    border-radius: 999px;
  }
  .stat-desc {
    font-size: 11.5px;
    color: var(--ts);
  }
  .risk-dots {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .risk-dot {
    width: 22px;
    height: 22px;
    border-radius: 7px;
    border: 1px solid var(--border);
  }
  .risk-dot.low { background: var(--accent-soft); }
  .risk-dot.med { background: var(--amber-soft); }
  
</style>
