<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let today = $state(new Date().toISOString().split('T')[0]);
  let selectedDate = $state(today);
  let logs = $state<any[]>([]);
  let loading = $state(true);

  let currentLog = $state<any>(null);

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
    currentLog = logs.find(l => l.log_date === selectedDate) || null;
  });

  let trendLogs = $derived([...logs].reverse());

  function getColor(val: number | null): string {
    if (val == null) return '#888';
    if (val > 7) return '#4caf50';
    if (val >= 5) return '#ff9800';
    return '#f44336';
  }

  function stageWidth(stage: number | null, total: number | null): number {
    if (stage == null || total == null || total === 0) return 0;
    return Math.min(100, Math.max(3, (stage / total) * 100));
  }
</script>

<h1>Sleep Tracking</h1>

<div class="date-picker">
  <label for="date">Date</label>
  <input type="date" id="date" bind:value={selectedDate} />
</div>

{#if loading}
  <p class="empty">Loading...</p>
{:else if !currentLog}
  <p class="empty">No sleep data recorded for this date.</p>
{:else}
  <div class="ratings-row">
    <div class="metric-card">
      <span class="metric-label">My Rating</span>
      <span class="metric-value">{currentLog.my_sleep_rating ?? '—'}</span>
      <span class="metric-scale">/ 10</span>
    </div>
    <div class="metric-card">
      <span class="metric-label">Phone Rating</span>
      <span class="metric-value">{currentLog.phone_sleep_rating ?? '—'}</span>
      <span class="metric-scale">/ 10</span>
    </div>
  </div>

  <div class="avg-card">
    <span class="avg-label">Average Sleep Rating</span>
    <span class="avg-value" style="color: {getColor(currentLog.sleep_avg)}">
      {currentLog.sleep_avg != null ? currentLog.sleep_avg.toFixed(1) : '—'}
    </span>
  </div>

  <div class="quality-bar-track">
    <div
      class="quality-bar-fill"
      style="width: {(currentLog.sleep_avg != null ? (currentLog.sleep_avg / 10) * 100 : 0)}%; background: {getColor(currentLog.sleep_avg)}"
    ></div>
  </div>

  <div class="section-title">Sleep Stages</div>
  {#if currentLog.sleep_actual_asleep != null && currentLog.sleep_actual_asleep > 0}
    <div class="stages-bar">
      <div class="stage-seg" style="width: {stageWidth(currentLog.sleep_rem, currentLog.sleep_actual_asleep)}%; background: #7c4dff;">
        <span class="stage-label">REM {currentLog.sleep_rem ?? 0}h</span>
      </div>
      <div class="stage-seg" style="width: {stageWidth(currentLog.sleep_deep, currentLog.sleep_actual_asleep)}%; background: #1976d2;">
        <span class="stage-label">Deep {currentLog.sleep_deep ?? 0}h</span>
      </div>
      <div class="stage-seg" style="width: {stageWidth(currentLog.sleep_awake, currentLog.sleep_actual_asleep)}%; background: #ff7043;">
        <span class="stage-label">Awake {currentLog.sleep_awake ?? 0}h</span>
      </div>
    </div>
  {:else}
    <p class="empty">No stage data for this date.</p>
  {/if}

  <div class="section-title">Time in Bed vs Asleep</div>
  <div class="pillow-row">
    <div class="pillow-card">
      <span class="pillow-label">Head on Pillow</span>
      <span class="pillow-value">{currentLog.sleep_time_head_on_pillow != null ? currentLog.sleep_time_head_on_pillow.toFixed(1) + 'h' : '—'}</span>
    </div>
    <div class="pillow-card">
      <span class="pillow-label">Actually Asleep</span>
      <span class="pillow-value">{currentLog.sleep_actual_asleep != null ? currentLog.sleep_actual_asleep.toFixed(1) + 'h' : '—'}</span>
    </div>
  </div>
{/if}

<div class="section-title">30-Day Trend</div>
{#if trendLogs.length === 0}
  <p class="empty">No trend data available.</p>
{:else}
  <div class="trend-chart">
    {#each trendLogs as log, i}
      {@const h = log.sleep_avg != null ? Math.round((log.sleep_avg / 10) * 120) : 0}
      <div class="trend-col" title="{log.log_date}: {log.sleep_avg != null ? log.sleep_avg.toFixed(1) : '—'}">
        <div
          class="trend-bar"
          style="height: {Math.max(4, h)}px; background: {getColor(log.sleep_avg)};"
        ></div>
        {#if i % 5 === 0 || i === trendLogs.length - 1}
          <span class="trend-label">{log.log_date ? log.log_date.slice(5) : ''}</span>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  h1 { margin-bottom: 16px; }

  .date-picker {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 24px;
  }
  .date-picker label { font-size: 14px; font-weight: 600; color: #555; }
  :global(.dark) .date-picker label { color: #bbb; }
  .date-picker input[type="date"] {
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 8px;
    font-size: 14px;
    font-family: inherit;
    background: #fff;
  }
  :global(.dark) .date-picker input[type="date"] {
    background: #1e2a45;
    border-color: #444;
    color: #e0e0e0;
  }

  .empty { color: #888; padding: 32px 0; text-align: center; font-size: 14px; }

  .ratings-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin-bottom: 16px;
  }

  .metric-card {
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  :global(.dark) .metric-card { background: #1e2a45; }

  .metric-label { font-size: 13px; font-weight: 600; color: #555; }
  :global(.dark) .metric-label { color: #bbb; }

  .metric-value { font-size: 36px; font-weight: 700; line-height: 1; }
  .metric-scale { font-size: 13px; color: #888; }

  .avg-card {
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    margin-bottom: 12px;
  }
  :global(.dark) .avg-card { background: #1e2a45; }

  .avg-label { font-size: 14px; font-weight: 600; color: #555; }
  :global(.dark) .avg-label { color: #bbb; }

  .avg-value { font-size: 42px; font-weight: 700; line-height: 1; }

  .quality-bar-track {
    height: 14px;
    background: #e0e0e0;
    border-radius: 7px;
    overflow: hidden;
    margin-bottom: 28px;
  }
  :global(.dark) .quality-bar-track { background: #333; }

  .quality-bar-fill {
    height: 100%;
    border-radius: 7px;
    transition: width 0.3s ease;
  }

  .section-title {
    font-size: 15px;
    font-weight: 700;
    margin-bottom: 12px;
    margin-top: 8px;
    color: #333;
  }
  :global(.dark) .section-title { color: #ccc; }

  .stages-bar {
    display: flex;
    height: 32px;
    border-radius: 8px;
    overflow: hidden;
    margin-bottom: 28px;
  }

  .stage-seg {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 40px;
    transition: width 0.3s ease;
  }

  .stage-label { font-size: 11px; font-weight: 700; color: #fff; white-space: nowrap; }

  .pillow-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin-bottom: 32px;
  }

  .pillow-card {
    background: #fff;
    border-radius: 12px;
    padding: 18px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  :global(.dark) .pillow-card { background: #1e2a45; }

  .pillow-label { font-size: 13px; font-weight: 600; color: #555; }
  :global(.dark) .pillow-label { color: #bbb; }

  .pillow-value { font-size: 28px; font-weight: 700; }

  .trend-chart {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 150px;
    padding: 12px 4px 0;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    overflow-x: auto;
  }
  :global(.dark) .trend-chart { background: #1e2a45; }

  .trend-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    flex-shrink: 0;
    width: 28px;
  }

  .trend-bar {
    width: 18px;
    border-radius: 4px 4px 0 0;
    transition: height 0.3s ease;
    flex-shrink: 0;
  }

  .trend-label {
    font-size: 9px;
    color: #888;
    margin-top: 4px;
    white-space: nowrap;
  }
  :global(.dark) .trend-label { color: #999; }
</style>
