<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate, formatDateShort, todayISO, shiftISO, fatigueBand } from '$lib/formatDate';
  import { computeDayLoad } from '$lib/load';

  let params = $state<any[]>([]);
  let predictions = $state<any[]>([]);
  // Full era history (for the charts) + actual next-day fatigue keyed by date.
  let allPredictions = $state<any[]>([]);
  let actualByDate = $state<Record<string, number>>({});
  let chartTab = $state<'time' | 'scatter'>('time');
  let loading = $state(true);
  let showAdvanced = $state(false);
  // Raw per-day activity load (same figures as the Activity page) for the
  // contribution bars; the risk math below still uses the model's scaled values.
  let rawLoad = $state({ phys: 0, cog: 0, sens: 0, total: 0 });

  // PEM is post-exertional: today's crash risk is driven by the load you carried
  // on the days BEFORE today. Today's own figures are empty until logged (often
  // not until tomorrow), so the page shows the most recent COMPLETE day —
  // yesterday — and presents it as today's risk, flagging the source date.
  const loadDate = shiftISO(todayISO(), -1);

  onMount(async () => {
    try {
      params = await invoke('get_calibration_params');
      await loadPredictions();
      // The per-page run only ever computes yesterday, so early on the history is
      // nearly empty and every row looks identical. Backfill once when it's sparse
      // so the list reflects real day-to-day variation; otherwise just ensure
      // yesterday exists so the breakdown isn't blank.
      if (predictions.length < 7) {
        await backfill();
      } else if (!predictions.find((p: any) => p.log_date === loadDate)) {
        await runModel();
      }
      // Actual fatigue scores (for the prediction-vs-actual charts & history column)
      // + raw activity load for the contribution bars (matches the Activity page).
      const [logs, acts, types, cats] = await Promise.all([
        invoke<any[]>('list_daily_logs', { limit: 500, offset: 0 }),
        invoke<any[]>('get_activities_for_date', { date: loadDate }),
        invoke<any[]>('list_activity_types', { categoryId: null }),
        invoke<any[]>('list_activity_categories'),
      ]);
      const m: Record<string, number> = {};
      for (const l of logs) if (l.fatigue_rating != null) m[l.log_date] = l.fatigue_rating;
      actualByDate = m;
      rawLoad = computeDayLoad(acts, types, cats);
    } catch (e) {
      console.error('PEM error:', e);
    } finally {
      loading = false;
    }
  });

  // Recent rows drive the history list; the full era feeds the charts.
  async function loadPredictions() {
    [predictions, allPredictions] = await Promise.all([
      invoke<any[]>('get_pem_predictions', { limit: 14 }),
      invoke<any[]>('get_pem_predictions', { limit: 400 }),
    ]);
  }

  async function runModel() {
    try {
      await invoke('run_pem_model', { date: loadDate });
      await loadPredictions();
    } catch (e) {
      console.error('Run model error:', e);
    }
  }

  // Recompute predictions for every logged day (not just yesterday).
  async function backfill() {
    try {
      await invoke('backfill_pem_predictions');
      await loadPredictions();
    } catch (e) {
      console.error('Backfill error:', e);
    }
  }

  // Actual next-day fatigue for a prediction made on `date` (the day after).
  function actualNextDay(date: string): number | null {
    return actualByDate[shiftISO(date, 1)] ?? null;
  }

  async function updateParam(name: string, value: number) {
    await invoke('update_calibration_param', { paramName: name, paramValue: value });
    params = params.map((p: any) => p.param_name === name ? { ...p, param_value: value } : p);
  }

  // The prediction driving today's risk (computed from yesterday's complete data).
  let todayPrediction = $derived(predictions.find((p: any) => p.log_date === loadDate));
  // Band reflects the predicted fatigue score (Low 0–3 / Med 3.1–6 / High 6.1–10),
  // matching the dashboard's headline number.
  let fatBand = $derived(fatigueBand(todayPrediction?.predicted_next_day_fatigue));

  // The model's own scaled load (steps/sleep blended in, each component ÷3) — this
  // is the real input to the weighted-load step, NOT the raw activity total above.
  let modelLoad = $derived(
    (todayPrediction?.physical_load ?? 0)
    + (todayPrediction?.cognitive_load ?? 0)
    + (todayPrediction?.sensory_social_load ?? 0)
  );

  // Contribution bars use the raw activity load (recognisable, matches Activity).
  let loads = $derived({
    phys: rawLoad.phys,
    cog: rawLoad.cog,
    sen: rawLoad.sens,
    total: rawLoad.total,
    max: Math.max(rawLoad.phys, rawLoad.cog, rawLoad.sens, 0.01),
  });

  // Only the snake_case params are read by the model. The spreadsheet import also
  // inserts a parallel set under human-readable names ("Steps weight (Physical
  // Load)" …) that nothing reads, so hide those — editing them does nothing.
  let modelParams = $derived(params.filter((p: any) => /^[a-z][a-z0-9_]*$/.test(p.param_name)));

  // ── Charts: predicted vs actual next-day fatigue ──
  // Full era history ascending, each prediction joined to the fatigue actually
  // logged the following day.
  let chartRows = $derived(
    [...allPredictions]
      .sort((a: any, b: any) => (a.log_date < b.log_date ? -1 : 1))
      .map((p: any) => ({
        date: p.log_date,
        predicted: p.predicted_next_day_fatigue as number | null,
        low: p.predicted_low as number | null,
        high: p.predicted_high as number | null,
        actual: actualNextDay(p.log_date),
      }))
  );

  // SVG plot box (viewBox units; the svg scales to its container width).
  const CW = 680, CH = 280, padL = 30, padR = 16, padT = 14, padB = 30;
  const plotW = CW - padL - padR;
  const plotH = CH - padT - padB;

  // Time chart: x = position in series, y = fatigue 0–10.
  function xAt(i: number, n: number): number {
    return padL + (n <= 1 ? plotW / 2 : (i / (n - 1)) * plotW);
  }
  function yAt(v: number): number {
    return padT + (1 - Math.max(0, Math.min(10, v)) / 10) * plotH;
  }
  function linePath(rows: any[], key: string): string {
    let d = '';
    let pen = false;
    rows.forEach((r, i) => {
      const v = r[key];
      if (v == null) { pen = false; return; }
      d += `${pen ? 'L' : 'M'}${xAt(i, rows.length).toFixed(1)} ${yAt(v).toFixed(1)} `;
      pen = true;
    });
    return d.trim();
  }
  // A handful of evenly spaced date labels for the x-axis.
  let timeXLabels = $derived.by(() => {
    const n = chartRows.length;
    if (n === 0) return [] as { x: number; label: string }[];
    const want = Math.min(6, n);
    const out: { x: number; label: string }[] = [];
    for (let k = 0; k < want; k++) {
      const i = Math.round((k / (want - 1 || 1)) * (n - 1));
      out.push({ x: xAt(i, n), label: formatDateShort(chartRows[i].date) });
    }
    return out;
  });

  // Scatter: predicted (x) vs actual next-day fatigue (y), both on a 1–10 scale.
  let scatterPts = $derived(
    chartRows.filter((r) => r.predicted != null && r.actual != null)
      .map((r) => [r.predicted as number, r.actual as number] as [number, number])
  );
  function sX(v: number): number { return padL + ((Math.max(1, Math.min(10, v)) - 1) / 9) * plotW; }
  function sY(v: number): number { return padT + (1 - (Math.max(1, Math.min(10, v)) - 1) / 9) * plotH; }
  // Ordinary-least-squares fit + R² for the trendline.
  let fit = $derived.by(() => {
    const pts = scatterPts;
    const n = pts.length;
    if (n < 2) return null;
    let sx = 0, sy = 0, sxx = 0, sxy = 0, syy = 0;
    for (const [x, y] of pts) { sx += x; sy += y; sxx += x * x; sxy += x * y; syy += y * y; }
    const denom = n * sxx - sx * sx;
    if (denom === 0) return null;
    const slope = (n * sxy - sx * sy) / denom;
    const intercept = (sy - slope * sx) / n;
    const r = (n * sxy - sx * sy) / Math.sqrt(denom * (n * syy - sy * sy));
    return { slope, intercept, r2: r * r };
  });

  // Format a number for display, or an em-dash when there's no prediction yet.
  function num(v: number | null | undefined, dp = 2): string {
    return v == null ? '—' : v.toFixed(dp);
  }

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

  // Fill arc for the gauge. The visible track is a 180° semicircle across the top
  // (from the left point 18,96 to the right point 152,96), so the fill must use the
  // same half-circle geometry — π (left) → 2π (right). The old version assumed a
  // 270° gauge and drew a floating arc that didn't sit on the track.
  function riskArc(score: number | null) {
    if (score == null) return '';
    const r = 67, cx = 85, cy = 96;
    const startAngle = Math.PI;        // left end of the semicircle
    const endAngle = Math.PI * 2;      // right end
    const pct = Math.min(1, Math.max(0, score / 10));
    const angle = startAngle + pct * (endAngle - startAngle);
    const sx = cx + r * Math.cos(startAngle);
    const sy = cy + r * Math.sin(startAngle);
    const ex = cx + r * Math.cos(angle);
    const ey = cy + r * Math.sin(angle);
    return `M${sx} ${sy} A${r} ${r} 0 0 1 ${ex} ${ey}`;
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">PEM Model</div>
    <div class="page-subtitle">Today's predicted fatigue, from your accumulated recovery debt through {formatDate(loadDate)}</div>
  </div>
  <div class="header-actions">
    <button class="ghost-btn" onclick={backfill}>Backfill history</button>
    <button class="run-btn" onclick={runModel}>Recalculate</button>
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
          <path d={riskArc(todayPrediction?.predicted_next_day_fatigue)} fill="none" stroke={fatBand === 'High' ? 'var(--red)' : fatBand === 'Medium' ? 'var(--amber)' : 'var(--accent)'} stroke-width="13" stroke-linecap="round"/>
        </svg>
        <div class="gauge-value">{todayPrediction?.predicted_next_day_fatigue?.toFixed(1) ?? '—'}</div>
        <div class="gauge-of">predicted · of 10</div>
      </div>
      <span class="risk-badge" style="color:{bandColor(fatBand)};background:{bandBg(fatBand)};">
        {fatBand ?? 'No data'} risk
      </span>
      <div class="risk-desc">
        {fatBand === 'High' ? 'High risk today — rest is essential.' :
         fatBand === 'Medium' ? 'A manageable day. The main pressure is load; recovery debt is still under the crash line.' :
         'Low risk today. Good to maintain your usual pacing.'}
      </div>
    </div>

    <div class="load-card">
      <div class="card-heading">Load contributions <span class="load-day">· yesterday {formatDate(loadDate)}</span></div>
      <div class="load-bars">
        <div class="load-item">
          <div class="load-header"><span><span class="load-swatch" style="background:var(--accent);"></span>Physical</span><span class="load-val">{num(loads.phys, 1)}</span></div>
          <div class="bar-track"><div class="bar-fill" style="width:{loads.phys / loads.max * 100}%;background:var(--accent);"></div></div>
        </div>
        <div class="load-item">
          <div class="load-header"><span><span class="load-swatch" style="background:var(--peri);"></span>Cognitive</span><span class="load-val">{num(loads.cog, 1)}</span></div>
          <div class="bar-track"><div class="bar-fill" style="width:{loads.cog / loads.max * 100}%;background:var(--peri);"></div></div>
        </div>
        <div class="load-item">
          <div class="load-header"><span><span class="load-swatch" style="background:var(--amber);"></span>Sensory / social</span><span class="load-val">{num(loads.sen, 1)}</span></div>
          <div class="bar-track"><div class="bar-fill" style="width:{loads.sen / loads.max * 100}%;background:var(--amber);"></div></div>
        </div>
      </div>
      <div class="load-tiles">
        <div class="load-tile">
          <div class="tile-label">Recovery debt</div>
          <div class="tile-val">{num(todayPrediction?.recovery_debt, 1)} <span class="tile-threshold">/ 4.0</span></div>
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
        <div class="step-label">1 · Scaled load</div>
        <div class="step-val">{num(modelLoad)}</div>
        <div class="step-desc">activity + steps/sleep, scaled</div>
      </div>
      <span class="step-op">→</span>
      <div class="step">
        <div class="step-label">2 · Weighted load</div>
        <div class="step-val">{num(todayPrediction?.three_day_weighted_load)}</div>
        <div class="step-desc">scaled load × 0.55</div>
      </div>
      <span class="step-op">→</span>
      <div class="step">
        <div class="step-label">3 · Recovery debt</div>
        <div class="step-val">{num(todayPrediction?.recovery_debt)}</div>
        <div class="step-desc">carried over + today's load</div>
      </div>
      <span class="step-op">→</span>
      <div class="step result">
        <div class="step-label">Predicted fatigue</div>
        <div class="step-val">{todayPrediction?.predicted_next_day_fatigue?.toFixed(1) ?? '—'}</div>
        <div class="step-desc" style="color:var(--amber-fg);font-weight:600;">{fatBand ?? '—'} · from recovery debt</div>
      </div>
    </div>
    
  </div>

  <div class="advanced-card">
    <button class="advanced-toggle" onclick={() => showAdvanced = !showAdvanced}>
      <div class="advanced-left">
        <span class="advanced-icon">
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8h16M4 16h16"/><circle cx="14" cy="8" r="2.4"/><circle cx="9" cy="16" r="2.4"/></svg>
        </span>
        <div>
          <div class="card-heading">Calibration parameters</div>
          <div class="card-subtitle">{modelParams.length} values tuning the load &amp; recovery-debt model · rarely changed</div>
        </div>
      </div>
      <span class="advanced-label">{showAdvanced ? 'Hide' : 'Show all 33'}</span>
    </button>
    {#if showAdvanced}
      <div class="advanced-content">
        <div class="param-grid">
          {#each modelParams as param}
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

  <div class="chart-card">
    <div class="chart-head">
      <div>
        <div class="card-heading">{chartTab === 'time' ? 'Predicted vs actual next-day fatigue' : 'Predicted vs actual — correlation'}</div>
        <div class="card-subtitle">{chartTab === 'time'
          ? 'How each day’s prediction (with its range) tracked the fatigue you actually logged the next day.'
          : 'Each dot is one day; the trendline shows how well the prediction lines up with reality.'}</div>
      </div>
      <div class="seg-control">
        <button class="seg-btn" class:active={chartTab === 'time'} onclick={() => chartTab = 'time'}>Over time</button>
        <button class="seg-btn" class:active={chartTab === 'scatter'} onclick={() => chartTab = 'scatter'}>Correlation</button>
      </div>
    </div>

    {#if chartRows.length === 0}
      <p class="empty-text">No predictions to chart yet — backfill history first.</p>
    {:else if chartTab === 'time'}
      <svg viewBox="0 0 {CW} {CH}" class="chart-svg" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Predicted vs actual next-day fatigue over time">
        {#each [0, 2, 4, 6, 8, 10] as g}
          <line x1={padL} y1={yAt(g)} x2={CW - padR} y2={yAt(g)} class="grid" />
          <text x={padL - 6} y={yAt(g) + 3} class="axis-label" text-anchor="end">{g}</text>
        {/each}
        <path d={linePath(chartRows, 'high')} class="band-line" />
        <path d={linePath(chartRows, 'low')} class="band-line" />
        <path d={linePath(chartRows, 'predicted')} class="pred-line" />
        <path d={linePath(chartRows, 'actual')} class="actual-line" />
        {#each timeXLabels as t}
          <text x={t.x} y={CH - padB + 17} class="axis-label" text-anchor="middle">{t.label}</text>
        {/each}
      </svg>
      <div class="chart-legend">
        <span class="leg"><span class="leg-line pred"></span>Predicted</span>
        <span class="leg"><span class="leg-line band"></span>Prediction range</span>
        <span class="leg"><span class="leg-line actual"></span>Actual next day</span>
      </div>
    {:else}
      <svg viewBox="0 0 {CW} {CH}" class="chart-svg" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Predicted vs actual next-day fatigue correlation">
        {#each [2, 4, 6, 8, 10] as g}
          <line x1={padL} y1={sY(g)} x2={CW - padR} y2={sY(g)} class="grid" />
          <text x={padL - 6} y={sY(g) + 3} class="axis-label" text-anchor="end">{g}</text>
          <text x={sX(g)} y={CH - padB + 17} class="axis-label" text-anchor="middle">{g}</text>
        {/each}
        {#if fit}
          <line x1={sX(1)} y1={sY(fit.intercept + fit.slope * 1)} x2={sX(10)} y2={sY(fit.intercept + fit.slope * 10)} class="trend" />
        {/if}
        {#each scatterPts as [px, py]}
          <circle cx={sX(px)} cy={sY(py)} r="3.6" class="dot" />
        {/each}
        <text x={CW - padR} y={padT + 12} class="fit-label" text-anchor="end">
          {#if fit}R² = {fit.r2.toFixed(2)} · y = {fit.slope.toFixed(2)}x + {fit.intercept.toFixed(1)}{:else}Not enough data{/if}
        </text>
      </svg>
      <div class="chart-legend">
        <span class="leg axis-note">x · predicted fatigue</span>
        <span class="leg axis-note">y · actual next-day fatigue</span>
      </div>
    {/if}
  </div>

  <div class="pred-card">
    <div class="card-heading" style="margin-bottom:12px;">Fatigue prediction</div>
    <div class="pred-row head">
      <span class="pred-date">Day</span>
      <span class="pred-crash"></span>
      <span class="pred-band">Band</span>
      <span class="pred-risk">Predicted</span>
      <span class="pred-actual">Actual</span>
    </div>
    {#each predictions as pred}
      {@const band = fatigueBand(pred.predicted_next_day_fatigue)}
      {@const actual = actualNextDay(pred.log_date)}
      <div class="pred-row" class:crash={pred.crash_flag}>
        <span class="pred-date">{formatDate(pred.log_date)}</span>
        <span class="pred-crash">{#if pred.crash_flag}<span class="crash-badge">⚠ Crash</span>{/if}</span>
        <span class="pred-band" style="color:{bandColor(band)};background:{bandBg(band)};">{band ?? '—'}</span>
        <span class="pred-risk"><strong>{pred.predicted_next_day_fatigue?.toFixed(1) ?? '—'}</strong></span>
        <span class="pred-actual"><strong>{actual?.toFixed(1) ?? '—'}</strong></span>
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
  .ghost-btn { display:inline-flex;align-items:center;gap:7px;background:transparent;color:var(--accent-fg);border:1px solid var(--border);border-radius:999px;padding:10px 16px;font-size:13px;font-weight:700;cursor:pointer; }
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
  .load-day { font-family:'Public Sans',sans-serif; font-size:12px; font-weight:500; color:var(--tm); }
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
  .pred-row.head { padding:4px 14px; margin-bottom:2px; }
  .pred-row.head span { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--tm); }
  .pred-date { font-size:13px; color:var(--ts); min-width:90px; flex:1; }
  .pred-crash { width:78px; flex-shrink:0; text-align:right; display:flex; justify-content:flex-end; align-items:center; }
  .pred-band { font-size:11px; padding:2px 8px; border-radius:4px; font-weight:600; min-width:62px; text-align:center; }
  .pred-row.head .pred-band { padding:0; background:transparent !important; text-align:left; }
  .pred-risk { font-size:14px; color:var(--tp); width:84px; text-align:right; }
  .pred-actual { font-size:14px; color:var(--ts); width:72px; text-align:right; }
  .pred-risk strong, .pred-actual strong { font-variant-numeric:tabular-nums; }
  .crash-badge { font-size:12px; color:var(--amber-fg); font-weight:700; }

  /* ── Predicted-vs-actual charts ── */
  .chart-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); margin-bottom:16px; }
  .chart-head { display:flex; justify-content:space-between; align-items:flex-start; gap:16px; margin-bottom:16px; flex-wrap:wrap; }
  .seg-control { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px; gap:2px; flex-shrink:0; }
  .seg-btn { background:transparent; border:none; border-radius:9px; padding:7px 13px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .seg-btn.active { background:var(--accent); color:#fff; }
  .chart-svg { width:100%; height:auto; display:block; }
  .chart-svg .grid { stroke:var(--border); stroke-width:1; }
  .chart-svg .axis-label { fill:var(--tm); font-size:10px; font-family:'Public Sans',sans-serif; }
  .chart-svg .fit-label { fill:var(--ts); font-size:11px; font-weight:700; font-family:'Public Sans',sans-serif; }
  .chart-svg .pred-line { fill:none; stroke:var(--accent); stroke-width:2; stroke-linejoin:round; stroke-linecap:round; }
  .chart-svg .actual-line { fill:none; stroke:var(--red); stroke-width:2; stroke-linejoin:round; stroke-linecap:round; }
  .chart-svg .band-line { fill:none; stroke:var(--tm); stroke-width:1; stroke-dasharray:3 3; opacity:.55; }
  .chart-svg .trend { stroke:var(--red); stroke-width:2; }
  .chart-svg .dot { fill:var(--accent); opacity:.7; }
  .chart-legend { display:flex; gap:18px; flex-wrap:wrap; margin-top:12px; padding-left:4px; }
  .leg { display:inline-flex; align-items:center; gap:7px; font-size:12px; color:var(--ts); }
  .leg-line { width:18px; height:0; border-top:2px solid; border-radius:2px; }
  .leg-line.pred { border-color:var(--accent); }
  .leg-line.actual { border-color:var(--red); }
  .leg-line.band { border-top:2px dashed var(--tm); opacity:.7; }
  .leg.axis-note { color:var(--tm); font-weight:600; }
  .empty-text { color:var(--ts); font-size:13px; padding:12px 0; }
</style>