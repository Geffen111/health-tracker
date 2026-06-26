<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let summary = $state<any>(null);
  let loading = $state(true);

  onMount(async () => {
    try {
      summary = await invoke('get_dashboard_summary');
    } catch (e) {
      console.error('Dashboard error:', e);
    } finally {
      loading = false;
    }
  });
</script>

<h1>Dashboard</h1>

{#if loading}
  <p>Loading...</p>
{:else if summary}
  <div class="metric-grid">
    <div class="metric-card fatigue">
      <div class="metric-label">Fatigue (30d)</div>
      <div class="metric-value">{summary.fatigue_last_30d?.toFixed(1) ?? '—'}</div>
      <div class="metric-sub">7d: {summary.fatigue_last_7d?.toFixed(1) ?? '—'}</div>
    </div>
    <div class="metric-card sleep">
      <div class="metric-label">Sleep (30d)</div>
      <div class="metric-value">{summary.sleep_last_30d?.toFixed(1) ?? '—'}</div>
      <div class="metric-sub">7d: {summary.sleep_last_7d?.toFixed(1) ?? '—'}</div>
    </div>
    <div class="metric-card steps">
      <div class="metric-label">Steps (30d)</div>
      <div class="metric-value">{Math.round(summary.steps_last_30d ?? 0).toLocaleString()}</div>
      <div class="metric-sub">7d: {Math.round(summary.steps_last_7d ?? 0).toLocaleString()}</div>
    </div>
    <div class="metric-card hr">
      <div class="metric-label">Resting HR</div>
      <div class="metric-value">{summary.avg_resting_hr?.toFixed(0) ?? '—'}</div>
      <div class="metric-sub">bpm</div>
    </div>
    <div class="metric-card pem">
      <div class="metric-label">PEM Risk</div>
      <div class="metric-value">{summary.current_risk_band ?? '—'}</div>
      <div class="metric-sub">Crashes (30d): {summary.crash_count_30d}</div>
    </div>
    <div class="metric-card headache">
      <div class="metric-label">Headache Days</div>
      <div class="metric-value">{summary.headache_days_30d}</div>
      <div class="metric-sub">of last 30 days</div>
    </div>
  </div>

  <div class="stats-row">
    <div class="stat-box">
      <span class="stat-label">Total entries</span>
      <span class="stat-value">{summary.date_count}</span>
    </div>
    <div class="stat-box">
      <span class="stat-label">Sick leave (30d)</span>
      <span class="stat-value">{summary.sick_leave_total?.toFixed(1) ?? '0'}h</span>
    </div>
  </div>
{:else}
  <p class="empty">No data yet. Start by importing your spreadsheet or adding a daily log entry.</p>
{/if}

<div class="action-links">
  <a href="/daily" class="action-btn">+ Add Today's Entry</a>
  <a href="/medication" class="action-btn">Manage Medications</a>
</div>

<style>
  h1 {
    margin-bottom: 24px;
    font-size: 28px;
  }

  .metric-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 16px;
    margin-bottom: 24px;
  }

  .metric-card {
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    transition: transform 0.15s;
  }

  .metric-card:hover {
    transform: translateY(-2px);
  }

  :global(.dark) .metric-card {
    background: #1e2a45;
  }

  .metric-label {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #666;
    margin-bottom: 8px;
  }

  :global(.dark) .metric-label {
    color: #999;
  }

  .metric-value {
    font-size: 32px;
    font-weight: 700;
  }

  .metric-sub {
    font-size: 13px;
    color: #888;
    margin-top: 4px;
  }

  .stats-row {
    display: flex;
    gap: 16px;
    margin-bottom: 24px;
  }

  .stat-box {
    background: #fff;
    border-radius: 12px;
    padding: 16px 24px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  :global(.dark) .stat-box {
    background: #1e2a45;
  }

  .stat-label {
    font-size: 12px;
    color: #666;
  }

  .stat-value {
    font-size: 24px;
    font-weight: 700;
  }

  .empty {
    text-align: center;
    color: #888;
    padding: 48px;
    font-size: 16px;
  }

  .action-links {
    display: flex;
    gap: 12px;
  }

  .action-btn {
    display: inline-block;
    padding: 12px 24px;
    background: #1976d2;
    color: #fff;
    border-radius: 8px;
    text-decoration: none;
    font-weight: 600;
    transition: background 0.15s;
  }

  .action-btn:hover {
    background: #1565c0;
  }
</style>