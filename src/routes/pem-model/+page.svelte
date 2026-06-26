<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let params = $state<any[]>([]);
  let predictions = $state<any[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      params = await invoke('get_calibration_params');
      predictions = await invoke('get_pem_predictions', { limit: 14 });
    } catch (e) {
      console.error('PEM error:', e);
    } finally {
      loading = false;
    }
  });

  async function runModel() {
    const today = new Date().toISOString().split('T')[0];
    try {
      await invoke('run_pem_model', { date: today });
      predictions = await invoke('get_pem_predictions', { limit: 14 });
    } catch (e) {
      console.error('Run model error:', e);
    }
  }

  async function updateParam(name: string, value: number) {
    await invoke('update_calibration_param', { paramName: name, paramValue: value });
    params = params.map(p => p.param_name === name ? { ...p, param_value: value } : p);
  }
</script>

<h1>PEM Model</h1>

<div class="toolbar">
  <button class="run-btn" onclick={runModel}>Run PEM Model for Today</button>
</div>

{#if loading}
  <p>Loading...</p>
{:else}
  <h2>Recent Predictions</h2>
  <div class="pred-list">
    {#each predictions as pred}
      <div class="pred-card" class:high={pred.risk_band === 'High'} class:crash={pred.crash_flag}>
        <span class="pred-date">{pred.log_date}</span>
        <span class="pred-band" class:band-low={pred.risk_band === 'Low'} class:band-med={pred.risk_band === 'Medium'} class:band-high={pred.risk_band === 'High'}>
          {pred.risk_band}
        </span>
        <span class="pred-value">Risk: {pred.predicted_pem_risk?.toFixed(2)}</span>
        <span class="pred-future">Next: {pred.predicted_next_day_fatigue?.toFixed(1)}</span>
        {#if pred.crash_flag}
          <span class="crash-badge">⚠️ CRASH</span>
        {/if}
      </div>
    {/each}
  </div>

  <h2>Calibration Parameters</h2>
  <div class="param-grid">
    {#each params as param}
      <div class="param-card">
        <label>{param.param_name}</label>
        <input
          type="number"
          step="0.01"
          value={param.param_value}
          onchange={(e: Event) => {
            const target = e.target as HTMLInputElement;
            updateParam(param.param_name, parseFloat(target.value));
          }}
        />
        {#if param.description}
          <small>{param.description}</small>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  h1 { margin-bottom: 8px; }
  h2 { margin: 24px 0 12px; font-size: 18px; }

  .toolbar { margin-bottom: 20px; }
  .run-btn {
    padding: 10px 24px;
    background: #2e7d32;
    color: #fff;
    border: none;
    border-radius: 8px;
    font-weight: 600;
    cursor: pointer;
  }
  .run-btn:hover { background: #1b5e20; }

  .pred-list { display: grid; gap: 6px; max-width: 600px; }

  .pred-card {
    display: flex;
    align-items: center;
    gap: 12px;
    background: #fff;
    border-radius: 8px;
    padding: 10px 14px;
    box-shadow: 0 1px 2px rgba(0,0,0,0.06);
  }
  :global(.dark) .pred-card { background: #1e2a45; }
  .pred-card.high { border-left: 3px solid #e53935; }
  .pred-card.crash { background: #fff3e0; }
  :global(.dark) .pred-card.crash { background: #3e2723; }

  .pred-date { font-size: 13px; color: #888; min-width: 90px; }
  .pred-band { font-size: 11px; padding: 2px 8px; border-radius: 4px; font-weight: 600; }
  .band-low { background: #e8f5e9; color: #2e7d32; }
  .band-med { background: #fff8e1; color: #f57f17; }
  .band-high { background: #ffebee; color: #c62828; }
  .pred-value { font-size: 14px; flex: 1; }
  .pred-future { font-size: 13px; color: #555; }
  .crash-badge { font-size: 12px; color: #e65100; font-weight: 700; }

  .param-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
    max-width: 800px;
  }

  .param-card {
    background: #fff;
    border-radius: 10px;
    padding: 14px;
    box-shadow: 0 1px 2px rgba(0,0,0,0.06);
  }
  :global(.dark) .param-card { background: #1e2a45; }

  .param-card label {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: #555;
    margin-bottom: 6px;
  }

  .param-card input {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
  }
  :global(.dark) .param-card input { background: #2a3a5c; border-color: #444; color: #e0e0e0; }

  .param-card small {
    display: block;
    font-size: 11px;
    color: #888;
    margin-top: 4px;
  }
</style>