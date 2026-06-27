<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate, todayISO } from '$lib/formatDate';

  let params = $state<any[]>([]);
  let predictions = $state<any[]>([]);
  let loading = $state(true);
  let showAdvanced = $state(false);

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
    const today = todayISO();
    try {
      await invoke('run_pem_model', { date: today });
      predictions = await invoke('get_pem_predictions', { limit: 14 });
    } catch (e) {
      console.error('Run model error:', e);
    }
  }

  async function updateParam(name: string, value: number) {
    await invoke('update_calibration_param', { paramName: name, paramValue: value });
    params = params.map((p: any) => p.param_name === name ? { ...p, param_value: value } : p);
  }

  let todayPrediction = $derived(predictions.find((p: any) => p.log_date === todayISO()));

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

  function riskArc(score: number | null) {
    if (score == null) return '';
    const r = 67, cx = 85, cy = 96;
    const startAngle = Math.PI * 0.75;
    const endAngle = Math.PI * 2.25;
    const pct = Math.min(1, score / 10);
    const angle = startAngle + pct * (endAngle - startAngle);
    const sx = cx + r * Math.cos(startAngle);
    const sy = cy + r * Math.sin(startAngle);
    const ex = cx + r * Math.cos(angle);
    const ey = cy + r * Math.sin(angle);
    const large = pct > 0.5 ? 1 : 0;
    return `M${sx} ${sy} A${r} ${r} 0 ${large} 1 ${ex} ${ey}`;
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">PEM Model</div>
    <div class="page-subtitle">What's driving today's crash risk · {formatDate(todayISO())}</div>
  </div>
  <div class="header-actions">
    <button class="run-btn" onclick={runModel}>Run PEM Model for Today</button>
  </div>
</div>

{#if loading}
  <p class="loading-text">Loading...</p>
{:else}
  <div class="hero-row">
    <div class="gauge-card">
      <div class="gauge">
        <svg viewBox="0 0 170 112" width="170" height="112">
          <path d="M18 96 A67 67 0 0 1 152 96" fill="none" stroke="var(--inset)" stroke-width="13" stroke-linecap="round"/>
          <path d={riskArc(todayPrediction?.predicted_pem_risk)} fill="none" stroke={todayPrediction?.risk_band === 'High' ? 'var(--red)' : todayPrediction?.risk_band === 'Medium' ? 'var(--amber)' : 'var(--accent)'} stroke-width="13" stroke-linecap="round"/>
        </svg>
        <div class="gauge-value">{todayPrediction?.predicted_pem_risk?.toFixed(1) ?? '—'}</div>
        <div class="gauge-of">predicted · of 10</div>
      </div>
      <span class="risk-badge" style="color:{bandColor(todayPrediction?.risk_band)};background:{bandBg(todayPrediction?.risk_band)};">
        {todayPrediction?.risk_band ?? 'No data'} risk
      </span>
      <div class="risk-desc">
        {todayPrediction?.risk_band === 'High' ? 'High risk today — rest is essential.' :
         todayPrediction?.risk_band === 'Medium' ? 'A manageable day. The main pressure is load; recovery debt is still under the crash line.' :
         'Low risk today. Good to maintain your usual pacing.'}
      </div>
    </div>

    <div class="load-card">
      <div class="card-heading">Today's load contributions</div>
      <div class="load-bars">
        <div class="load-item">
          <div class="load-header"><span><span class="load-swatch" style="background:var(--accent);"></span>Physical</span><span class="load-val">—</span></div>
          <div class="bar-track"><div class="bar-fill" style="width:0%;background:var(--accent);"></div></div>
        </div>
        <div class="load-item">
          <div class="load-header"><span><span class="load-swatch" style="background:var(--peri);"></span>Cognitive</span><span class="load-val">—</span></div>
          <div class="bar-track"><div class="bar-fill" style="width:0%;background:var(--peri);"></div></div>
        </div>
        <div class="load-item">
          <div class="load-header"><span><span class="load-swatch" style="background:var(--amber);"></span>Sensory / social</span><span class="load-val">—</span></div>
          <div class="bar-track"><div class="bar-fill" style="width:0%;background:var(--amber);"></div></div>
        </div>
      </div>
      <div class="load-tiles">
        <div class="load-tile">
          <div class="tile-label">Recovery debt</div>
          <div class="tile-val">{todayPrediction?.predicted_pem_risk?.toFixed(1) ?? '—'} <span class="tile-threshold">/ 4.0</span></div>
        </div>
        <div class="load-tile">
          <div class="tile-label">Crash flag</div>
          <div class="tile-val" style="color:var(--accent-fg);">{todayPrediction?.crash_flag ? '⚠ Active' : 'None'}</div>
        </div>
      </div>
    </div>
  </div>

  <div class="how-card">
    <div class="card-heading" style="margin-bottom:4px;">How today's risk is built</div>
    <div class="card-subtitle" style="margin-bottom:18px;">Each step in plain terms — no need to read the formulas.</div>
    <div class="step-row">
      <div class="step">
        <div class="step-label">1 · Total load</div>
        <div class="step-val">—</div>
        <div class="step-desc">physical + cognitive + sensory</div>
      </div>
      <span class="step-op">×</span>
      <div class="step">
        <div class="step-label">2 · Sensitivity</div>
        <div class="step-val">—</div>
        <div class="step-desc">higher when fatigue is high</div>
      </div>
      <span class="step-op">+</span>
      <div class="step">
        <div class="step-label">3 · Recovery debt</div>
        <div class="step-val">—</div>
        <div class="step-desc">under the crash line</div>
      </div>
      <span class="step-op">→</span>
      <div class="step result">
        <div class="step-label">Predicted risk</div>
        <div class="step-val">{todayPrediction?.predicted_pem_risk?.toFixed(1) ?? '—'}</div>
        <div class="step-desc" style="color:var(--amber-fg);font-weight:600;">{todayPrediction?.risk_band ?? '—'} · pace gently</div>
      </div>
    </div>
    <div class="step-note">Tomorrow's predicted fatigue from this risk is about <strong>{todayPrediction?.predicted_next_day_fatigue?.toFixed(1) ?? '—'} / 10</strong>.</div>
  </div>

  <div class="advanced-card">
    <button class="advanced-toggle" onclick={() => showAdvanced = !showAdvanced}>
      <div class="advanced-left">
        <span class="advanced-icon">
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8h16M4 16h16"/><circle cx="14" cy="8" r="2.4"/><circle cx="9" cy="16" r="2.4"/></svg>
        </span>
        <div>
          <div class="card-heading">Calibration parameters</div>
          <div class="card-subtitle">33 values tuned to match the spreadsheet model · rarely changed</div>
        </div>
      </div>
      <span class="advanced-label">{showAdvanced ? 'Hide' : 'Show all 33'}</span>
    </button>
    {#if showAdvanced}
      <div class="advanced-content">
        <div class="param-grid">
          {#each params as param}
            <div class="param-row">
              <span class="param-name">{param.param_name}</span>
              <input type="number" step="0.01" value={param.param_value}
                onchange={(e: Event) => {
                  const target = e.target as HTMLInputElement;
                  updateParam(param.param_name, parseFloat(target.value));
                }}
                class="param-input"
              />
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="pred-card">
    <div class="card-heading" style="margin-bottom:12px;">Recent predictions</div>
    {#each predictions as pred}
      <div class="pred-row" class:crash={pred.crash_flag}>
        <span class="pred-date">{formatDate(pred.log_date)}</span>
        <span class="pred-band" style="color:{bandColor(pred.risk_band)};background:{bandBg(pred.risk_band)};">{pred.risk_band}</span>
        <span class="pred-risk">Risk: {pred.predicted_pem_risk?.toFixed(2)}</span>
        <span class="pred-future">Next: {pred.predicted_next_day_fatigue?.toFixed(1)}</span>
        {#if pred.crash_flag}
          <span class="crash-badge">⚠ Crash</span>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:22px; gap:16px; flex-wrap:wrap; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }
  .header-actions { display:flex; align-items:center; gap:10px; }
  .run-btn { display:inline-flex;align-items:center;gap:7px;background:var(--accent);color:#fff;border:none;border-radius:999px;padding:10px 16px;font-size:13px;font-weight:700;cursor:pointer; }
  .loading-text { color:var(--ts); text-align:center; padding:32px; }

  .hero-row { display:grid; grid-template-columns:1fr 1.6fr; gap:16px; margin-bottom:16px; }
  .gauge-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:24px; box-shadow:var(--shadow); display:flex; flex-direction:column; align-items:center; justify-content:center; gap:10px; text-align:center; }
  .gauge { position:relative; width:170px; height:112px; }
  .gauge-value { position:absolute; left:0;right:0;top:46px;text-align:center; font-family:'Source Serif 4',serif; font-size:44px; font-weight:600; color:var(--tp); letter-spacing:-.02em; }
  .gauge-of { position:absolute; left:0;right:0;top:98px;text-align:center; font-size:10.5px; color:var(--tm); font-weight:600; }
  .risk-badge { font-size:12px; font-weight:800; padding:5px 14px; border-radius:999px; }
  .risk-desc { font-size:12.5px; color:var(--ts); line-height:1.5; }

  .load-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:16px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:16px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); }

  .load-bars { display:flex; flex-direction:column; gap:13px; }
  .load-item { display:flex; flex-direction:column; gap:6px; }
  .load-header { display:flex; justify-content:space-between; font-size:12.5px; }
  .load-header span { color:var(--tp); font-weight:600; display:inline-flex; align-items:center; gap:7px; }
  .load-swatch { width:9px;height:9px;border-radius:3px;flex-shrink:0; }
  .load-val { color:var(--ts); font-weight:700; font-variant-numeric:tabular-nums; }
  .bar-track { height:9px; border-radius:999px; background:var(--inset); overflow:hidden; }
  .bar-fill { height:100%; border-radius:999px; }

  .load-tiles { display:flex; gap:12px; flex-wrap:wrap; }
  .load-tile { flex:1; min-width:120px; background:var(--inset); border-radius:13px; padding:12px 14px; }
  .tile-label { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .tile-val { font-family:'Source Serif 4',serif; font-size:22px; font-weight:600; color:var(--tp); }
  .tile-threshold { font-size:12px; color:var(--tm); }

  .how-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); margin-bottom:16px; }
  .step-row { display:flex; align-items:stretch; gap:10px; flex-wrap:wrap; }
  .step { flex:1; min-width:128px; background:var(--inset); border-radius:14px; padding:14px; display:flex; flex-direction:column; gap:5px; }
  .step.result { background:var(--amber-soft); border:1px solid var(--border); }
  .step-label { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .step.result .step-label { color:var(--amber-fg); }
  .step-val { font-family:'Source Serif 4',serif; font-size:24px; font-weight:600; color:var(--tp); }
  .step-desc { font-size:11px; color:var(--tm); line-height:1.4; }
  .step-op { display:flex; align-items:center; color:var(--tm); font-size:18px; }
  .step-note { margin-top:16px; font-size:12.5px; color:var(--ts); line-height:1.6; background:var(--inset); border-radius:12px; padding:13px 15px; }
  .step-note strong { color:var(--tp); }

  .advanced-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; margin-bottom:16px; }
  .advanced-toggle { width:100%; display:flex; align-items:center; justify-content:space-between; gap:12px; padding:18px 22px; background:transparent; border:none; cursor:pointer; text-align:left; }
  .advanced-left { display:flex; align-items:center; gap:12px; }
  .advanced-icon { width:34px;height:34px;border-radius:10px;background:var(--inset);display:flex;align-items:center;justify-content:center;color:var(--ts); }
  .advanced-label { font-size:12.5px; font-weight:700; color:var(--accent-fg); white-space:nowrap; }
  .advanced-content { padding:4px 22px 20px; border-top:1px solid var(--border); }
  .param-grid { display:grid; grid-template-columns:1fr 1fr 1fr; gap:8px 18px; margin-top:14px; }
  .param-row { display:flex; justify-content:space-between; align-items:center; gap:10px; padding:8px 0; border-bottom:1px solid var(--border); }
  .param-name { font-size:12px; color:var(--ts); font-family:'Public Sans',monospace; }
  .param-input { width:70px; font-size:12.5px; color:var(--tp); font-weight:700; font-variant-numeric:tabular-nums; background:var(--inset); border:1px solid var(--border); border-radius:6px; padding:4px 6px; text-align:right; }

  .pred-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); }
  .pred-row { display:flex; align-items:center; gap:12px; padding:10px 14px; border-radius:8px; margin-bottom:4px; background:var(--card); }
  .pred-row.crash { background:var(--amber-soft); }
  .pred-date { font-size:13px; color:var(--ts); min-width:90px; }
  .pred-band { font-size:11px; padding:2px 8px; border-radius:4px; font-weight:600; }
  .pred-risk { font-size:14px; flex:1; color:var(--tp); }
  .pred-future { font-size:13px; color:var(--ts); }
  .crash-badge { font-size:12px; color:var(--amber-fg); font-weight:700; }
</style>