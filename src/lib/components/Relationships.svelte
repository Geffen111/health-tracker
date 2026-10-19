<script lang="ts">
  // Signal check, generalised: how strongly each tracked measure has gone with any other
  // one — fatigue, deep sleep, resting HR — on the same day or the next. Medications and
  // food flags/categories are on/off days, compared as "days with" vs "days without".
  // Descriptive only, like the rest of Pacing: no scores, no forecasts, and a correlation
  // here is not a cause (a med started in a bad month carries that month with it).
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateShort, shiftISO } from '$lib/formatDate';
  import Chart from '$lib/Chart.svelte';
  import { features } from '$lib/stores/features.svelte';
  import { recallView, rememberView, oneOf } from '$lib/viewState';

  interface DayAgg { hours: number; load: number; high: number }
  interface FoodItem { id: number; category_id: number | null; flag_ids: number[] }
  interface FoodTag { id: number; name: string }

  let { logs, dayAgg, foodItems, foodDays, foodCategories, foodFlags } = $props<{
    logs: any[];
    dayAgg: Map<string, DayAgg>;
    foodItems: FoodItem[];
    foodDays: { food_id: number; log_date: string }[];
    foodCategories: FoodTag[];
    foodFlags: FoodTag[];
  }>();

  let meds = $state<{ id: number; name: string }[]>([]);
  let doses = $state<{ medication_id: number; log_date: string }[]>([]);
  let bp = $state<{ log_date: string; systolic: number; diastolic: number }[]>([]);

  onMount(async () => {
    const [m, d, b] = await Promise.all([
      features.medication ? invoke<any[]>('list_medications').catch(() => []) : [],
      features.medication ? invoke<any[]>('get_all_doses').catch(() => []) : [],
      features.cardio ? invoke<any>('get_bp_series').then((s) => s.adjusted).catch(() => []) : [],
    ]);
    meds = m; doses = d; bp = b;
  });

  type Group = 'Symptoms' | 'Sleep' | 'Body' | 'Activity' | 'Medication' | 'Food';
  interface Measure {
    key: string;
    label: string;
    group: Group;
    /** 'bin' = an on/off day (1 / 0), null where the day can't say either way. */
    kind: 'num' | 'bin';
    value: (day: string) => number | null | undefined;
  }

  const ON = 1, OFF = 0;

  let measures = $derived.by((): Measure[] => {
    const byDate = new Map<string, any>(logs.map((l: any) => [l.log_date, l]));
    const col = (c: string) => (d: string) => byDate.get(d)?.[c] ?? null;
    const out: Measure[] = [
      { key: 'fatigue', label: 'Fatigue', group: 'Symptoms', kind: 'num', value: col('fatigue_rating') },
      { key: 'headache', label: 'Headache', group: 'Symptoms', kind: 'num', value: col('headache_rating') },
    ];
    if (features.sleep) out.push(
      { key: 'sleep_score', label: 'Sleep score', group: 'Sleep', kind: 'num',
        value: (d) => byDate.get(d)?.sleep_avg ?? byDate.get(d)?.my_sleep_rating ?? null },
      { key: 'my_sleep', label: 'My sleep rating', group: 'Sleep', kind: 'num', value: col('my_sleep_rating') },
      { key: 'asleep', label: 'Time asleep', group: 'Sleep', kind: 'num', value: col('sleep_actual_asleep') },
      { key: 'deep', label: 'Deep sleep', group: 'Sleep', kind: 'num', value: col('sleep_deep') },
      { key: 'rem', label: 'REM sleep', group: 'Sleep', kind: 'num', value: col('sleep_rem') },
      { key: 'awake', label: 'Awake in bed', group: 'Sleep', kind: 'num', value: col('sleep_awake') },
    );
    out.push(
      { key: 'steps', label: 'Steps', group: 'Body', kind: 'num', value: col('steps') },
      { key: 'calories', label: 'Active calories', group: 'Body', kind: 'num', value: col('activity_calories') },
    );
    if (features.cardio) {
      const sys = new Map<string, number[]>(), dia = new Map<string, number[]>();
      for (const p of bp) {
        sys.set(p.log_date, [...(sys.get(p.log_date) ?? []), p.systolic]);
        dia.set(p.log_date, [...(dia.get(p.log_date) ?? []), p.diastolic]);
      }
      const dayMean = (m: Map<string, number[]>) => (d: string) => {
        const v = m.get(d);
        return v ? v.reduce((a, b) => a + b, 0) / v.length : null;
      };
      out.push(
        { key: 'rhr', label: 'Resting HR', group: 'Body', kind: 'num', value: col('ave_resting_hr') },
        { key: 'hr', label: 'Average HR', group: 'Body', kind: 'num', value: col('ave_hr') },
        { key: 'sys', label: 'Systolic BP', group: 'Body', kind: 'num', value: dayMean(sys) },
        { key: 'dia', label: 'Diastolic BP', group: 'Body', kind: 'num', value: dayMean(dia) },
      );
    }
    out.push({ key: 'alcohol', label: 'Alcohol (std drinks)', group: 'Body', kind: 'num', value: col('alcohol_std_drinks') });
    if (features.activity) out.push(
      { key: 'act_hours', label: 'Activity hours', group: 'Activity', kind: 'num', value: (d) => dayAgg.get(d)?.hours },
      { key: 'act_load', label: 'Activity load', group: 'Activity', kind: 'num', value: (d) => dayAgg.get(d)?.load },
      { key: 'act_high', label: 'High-energy hours', group: 'Activity', kind: 'num', value: (d) => dayAgg.get(d)?.high },
    );
    if (features.work) out.push({
      key: 'work', label: 'Work hours', group: 'Activity', kind: 'num',
      value: (d) => {
        const l = byDate.get(d);
        return l && (l.office_hours != null || l.wfh_hours != null) ? (l.office_hours ?? 0) + (l.wfh_hours ?? 0) : null;
      },
    });

    // A med is "on" for a day with one of its doses logged, "off" for a day with other
    // doses logged but none of it, and unknown on a day with no doses logged at all.
    if (features.medication) {
      const taken = new Map<string, Set<number>>();
      for (const d of doses) {
        if (!taken.has(d.log_date)) taken.set(d.log_date, new Set());
        taken.get(d.log_date)!.add(d.medication_id);
      }
      for (const m of meds) {
        if (!doses.some((d) => d.medication_id === m.id)) continue;
        out.push({
          key: `med:${m.id}`, label: `On ${m.name}`, group: 'Medication', kind: 'bin',
          value: (d) => { const s = taken.get(d); return s ? (s.has(m.id) ? ON : OFF) : null; },
        });
      }
    }

    // Same for food: "off" only on a day something was logged.
    if (features.food) {
      const items = new Map<number, FoodItem>(foodItems.map((f: FoodItem) => [f.id, f]));
      const tagsByDay = new Map<string, Set<string>>();
      for (const fd of foodDays) {
        const f = items.get(fd.food_id);
        if (!tagsByDay.has(fd.log_date)) tagsByDay.set(fd.log_date, new Set());
        const s = tagsByDay.get(fd.log_date)!;
        if (f?.category_id != null) s.add(`cat:${f.category_id}`);
        for (const id of f?.flag_ids ?? []) s.add(`flag:${id}`);
      }
      const tagMeasure = (key: string, label: string): Measure => ({
        key: `food:${key}`, label, group: 'Food', kind: 'bin',
        value: (d) => { const s = tagsByDay.get(d); return s ? (s.has(key) ? ON : OFF) : null; },
      });
      for (const g of foodFlags) out.push(tagMeasure(`flag:${g.id}`, g.name));
      for (const c of foodCategories) out.push(tagMeasure(`cat:${c.id}`, `${c.name} (category)`));
    }
    return out;
  });

  // ── Controls (remembered, see $lib/viewState) ──
  const saved = recallView<any>('relationships');
  let targetKey = $state<string>(typeof saved.targetKey === 'string' ? saved.targetKey : 'fatigue');
  let lag = $state<0 | 1>(oneOf(saved.lag, [0, 1] as const, 1));
  let groupFilter = $state<'All' | Group>(oneOf(saved.groupFilter, ['All', 'Symptoms', 'Sleep', 'Body', 'Activity', 'Medication', 'Food'] as const, 'All'));
  let showAll = $state(false);
  let detailKey = $state<string | null>(null);
  $effect(() => rememberView('relationships', { targetKey, lag, groupFilter }));

  let outcomes = $derived(measures.filter((m) => m.kind === 'num'));
  let target = $derived(outcomes.find((m) => m.key === targetKey) ?? outcomes[0]);

  // Every date anything was recorded on.
  let allDates = $derived.by(() => {
    const s = new Set<string>(logs.map((l: any) => l.log_date));
    for (const d of dayAgg.keys()) s.add(d);
    return [...s].sort();
  });

  function pearson(xs: number[], ys: number[]): number | null {
    const n = xs.length;
    if (n < 10) return null;
    const mx = xs.reduce((a, b) => a + b, 0) / n, my = ys.reduce((a, b) => a + b, 0) / n;
    let num = 0, dx = 0, dy = 0;
    for (let i = 0; i < n; i++) {
      num += (xs[i] - mx) * (ys[i] - my);
      dx += (xs[i] - mx) ** 2;
      dy += (ys[i] - my) ** 2;
    }
    return dx && dy ? num / Math.sqrt(dx * dy) : null;
  }
  const avg = (v: number[]) => (v.length ? v.reduce((a, b) => a + b, 0) / v.length : null);

  interface Row {
    m: Measure;
    r: number | null;
    n: number;
    xs: number[];
    ys: number[];
    days: string[];
    // On/off measures: the outcome's mean on days with and without, and how many of each.
    withMean?: number | null;
    withoutMean?: number | null;
    nWith?: number;
    nWithout?: number;
  }

  // An on/off measure needs a few days on each side before its r means anything.
  const MIN_SIDE = 5;

  function pairUp(m: Measure): Row {
    const xs: number[] = [], ys: number[] = [], days: string[] = [];
    for (const d of allDates) {
      const x = m.value(d);
      const y = target.value(lag ? shiftISO(d, lag) : d);
      if (x != null && y != null) { xs.push(Number(x)); ys.push(Number(y)); days.push(d); }
    }
    const row: Row = { m, r: pearson(xs, ys), n: xs.length, xs, ys, days };
    if (m.kind === 'bin') {
      const on = ys.filter((_, i) => xs[i] === ON), off = ys.filter((_, i) => xs[i] === OFF);
      row.withMean = avg(on); row.withoutMean = avg(off);
      row.nWith = on.length; row.nWithout = off.length;
      if (on.length < MIN_SIDE || off.length < MIN_SIDE) row.r = null;
    }
    return row;
  }

  let rows = $derived.by(() => {
    if (!target) return [];
    // Same-day, a measure against itself is trivially 1; next-day it's how a day carries over.
    return measures
      .filter((m) => lag === 1 || m.key !== target.key)
      .filter((m) => groupFilter === 'All' || m.group === groupFilter)
      .map(pairUp)
      .sort((a, b) => (b.r == null ? -1 : Math.abs(b.r)) - (a.r == null ? -1 : Math.abs(a.r)));
  });
  let shownRows = $derived(showAll ? rows : rows.slice(0, 12));
  let detail = $derived(rows.find((r) => r.m.key === detailKey) ?? null);

  function rowLabel(m: Measure): string {
    return lag === 1 && m.key === target?.key ? `${m.label} (that day)` : m.label;
  }

  function strength(r: number | null): { text: string; color: string } {
    if (r == null) return { text: 'not enough data', color: 'var(--tm)' };
    const a = Math.abs(r);
    if (a < 0.15) return { text: 'no signal', color: 'var(--tm)' };
    if (a < 0.3) return { text: 'very weak', color: 'var(--ts)' };
    if (a < 0.5) return { text: 'weak', color: 'var(--amber-fg)' };
    return { text: 'moderate', color: 'var(--accent-fg)' };
  }
  const fmt = (v: number | null | undefined) =>
    v == null ? '—' : Math.abs(v) >= 100 ? Math.round(v).toLocaleString() : v.toFixed(Math.abs(v) >= 10 ? 0 : 1);

  // ── Detail chart ──
  // On/off: the outcome over time, with the "on" days shaded behind it.
  // Numbers: one dot per day pair.
  let detailChart = $derived.by(() => {
    if (!detail || !target) return null;
    const yLabel = lag ? `${target.label} (next day)` : target.label;
    if (detail.m.kind === 'bin') {
      const labels = detail.days.map((d) => formatDateShort(d));
      const ymax = Math.max(...detail.ys, 1);
      return {
        type: 'bar',
        labels,
        datasets: [
          { type: 'line', label: yLabel, data: detail.ys, borderColor: 'var(--accent)', backgroundColor: 'var(--accent)',
            borderWidth: 1.5, pointRadius: detail.days.length > 120 ? 0 : 1.5, spanGaps: true, yAxisID: 'y', order: 1 },
          { type: 'bar', label: detail.m.label, data: detail.xs.map((x) => (x === ON ? ymax : null)),
            backgroundColor: 'var(--amber-soft)', borderWidth: 0, barPercentage: 1, categoryPercentage: 1, yAxisID: 'y', order: 2 },
        ],
        options: {
          interaction: { mode: 'index', intersect: false },
          scales: {
            x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 }, maxTicksLimit: 8 } },
            y: { beginAtZero: true, grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } } },
          },
          plugins: {
            legend: { display: true, position: 'bottom', labels: { color: 'var(--ts)', font: { size: 11 }, boxWidth: 10 } },
            tooltip: { filter: (item: any) => item.datasetIndex === 0 },
          },
        },
      };
    }
    return {
      type: 'scatter',
      labels: [],
      datasets: [{
        label: `${detail.m.label} vs ${yLabel}`,
        data: detail.xs.map((x, i) => ({ x, y: detail!.ys[i] })),
        backgroundColor: 'var(--accent)', pointRadius: 3, pointHoverRadius: 5,
      }],
      options: {
        scales: {
          x: { grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 10 } },
               title: { display: true, text: detail.m.label, color: 'var(--tm)', font: { size: 11 } } },
          y: { grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } },
               title: { display: true, text: yLabel, color: 'var(--tm)', font: { size: 11 } } },
        },
        plugins: { legend: { display: false } },
      },
    };
  });
</script>

<div class="card">
  <div class="card-head">
    <div>
      <div class="card-heading">Signal check</div>
      <div class="card-subtitle">
        How strongly each measure has gone with
        {lag ? 'the next day’s' : 'the same day’s'} <strong>{target?.label.toLowerCase() ?? '—'}</strong>, across your whole log.
        Medications and food are on/off days, compared as days with against days without. Click a row to see it.
      </div>
    </div>
    <div class="controls">
      <select bind:value={targetKey} aria-label="Compare against">
        {#each outcomes as o (o.key)}<option value={o.key}>{o.label}</option>{/each}
      </select>
      <div class="seg-control">
        <button class="seg-btn" class:active={lag === 0} onclick={() => lag = 0}>Same day</button>
        <button class="seg-btn" class:active={lag === 1} onclick={() => lag = 1}>Next day</button>
      </div>
    </div>
  </div>

  <div class="chips">
    {#each ['All', 'Symptoms', 'Sleep', 'Body', 'Activity', 'Medication', 'Food'] as g}
      {#if g === 'All' || measures.some((m) => m.group === g)}
        <button class="chip" class:on={groupFilter === g} onclick={() => { groupFilter = g as any; showAll = false; }}>{g}</button>
      {/if}
    {/each}
  </div>

  <div class="signal-grid">
    {#each shownRows as s (s.m.key)}
      {@const st = strength(s.r)}
      <button class="signal-row" class:open={detailKey === s.m.key} onclick={() => detailKey = detailKey === s.m.key ? null : s.m.key}>
        <span class="signal-label">{rowLabel(s.m)}</span>
        <div class="signal-bar-track">
          <div class="signal-bar" style="width:{Math.min(100, Math.abs(s.r ?? 0) * 200)}%;background:{st.color};"></div>
        </div>
        <span class="signal-r">{s.r == null ? '—' : (s.r > 0 ? '+' : '') + s.r.toFixed(2)}</span>
        <span class="signal-strength" style="color:{st.color};">{st.text}</span>
        <span class="signal-n">
          {#if s.m.kind === 'bin'}
            with {fmt(s.withMean)} · without {fmt(s.withoutMean)} <span class="muted">({s.nWith}/{s.nWithout} days)</span>
          {:else}
            <span class="muted">{s.n} days</span>
          {/if}
        </span>
      </button>
      {#if detailKey === s.m.key && detailChart}
        <div class="detail">
          <Chart type={detailChart.type} labels={detailChart.labels} datasets={detailChart.datasets} options={detailChart.options} chartArea="220px" />
        </div>
      {/if}
    {/each}
  </div>
  {#if rows.length > 12}
    <button class="chip clear" onclick={() => showAll = !showAll}>{showAll ? 'Show fewer' : `Show all ${rows.length}`}</button>
  {/if}

  <p class="signal-note">
    r runs from −1 to +1: the sign is the direction (for sleep and steps, + means more goes with a higher
    {target?.label.toLowerCase() ?? 'value'}), the size how closely they move together. With this many
    measures a few will look related by chance, so read the weak ones as leads, not findings. Things that
    changed at the same time (a new med in a bad month) get each other's credit.
    {#if target?.key === 'fatigue' && lag === 1}
      This replaced the old PEM risk score and predicted-fatigue gauge. Measured over 102 day-pairs,
      that model scored no better than always guessing your average (RMSE 1.88 vs 1.90), and none of
      its exertion inputs correlated with the next day. Low activity tends to <em>follow</em> a bad day
      rather than precede one — a weak reading here means daily totals are too coarse to see pacing,
      not that it doesn't matter.
    {/if}
  </p>
</div>

<style>
  .card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); margin-bottom:16px; }
  .card-head { display:flex; justify-content:space-between; align-items:flex-start; gap:16px; margin-bottom:14px; flex-wrap:wrap; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:16px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; max-width:64ch; line-height:1.5; }
  .controls { display:flex; gap:8px; flex-wrap:wrap; align-items:center; }
  select { background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:8px 10px; font-size:12.5px; font-weight:600; color:var(--tp); font-family:inherit; }
  .seg-control { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px; gap:2px; flex-shrink:0; }
  .seg-btn { background:transparent; border:none; border-radius:9px; padding:7px 12px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .seg-btn.active { background:var(--accent); color:#fff; }
  .chips { display:flex; flex-wrap:wrap; gap:6px; margin-bottom:14px; }
  .chip { background:var(--inset); border:1px solid var(--border); border-radius:999px; padding:5px 11px; font-size:11.5px; font-weight:600; color:var(--ts); cursor:pointer; font-family:inherit; }
  .chip.on { background:var(--accent); border-color:var(--accent); color:#fff; }
  .chip.clear { background:transparent; color:var(--accent-fg); font-weight:700; margin-top:10px; }

  .signal-grid { display:flex; flex-direction:column; gap:4px; }
  .signal-row { display:grid; grid-template-columns:170px 1fr 52px 100px minmax(170px, auto); align-items:center; gap:12px;
                background:none; border:none; padding:5px 8px; margin:0 -8px; border-radius:8px; cursor:pointer; text-align:left; font-family:inherit; }
  .signal-row:hover, .signal-row.open { background:var(--inset); }
  .signal-label { font-size:12.5px; color:var(--tp); font-weight:600; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .signal-bar-track { height:8px; border-radius:999px; background:var(--inset); overflow:hidden; }
  .signal-row:hover .signal-bar-track, .signal-row.open .signal-bar-track { background:var(--card); }
  .signal-bar { height:100%; border-radius:999px; min-width:2px; }
  .signal-r { font-size:12.5px; color:var(--ts); font-weight:700; font-variant-numeric:tabular-nums; text-align:right; }
  .signal-strength { font-size:11.5px; font-weight:700; }
  .signal-n { font-size:11.5px; color:var(--ts); font-variant-numeric:tabular-nums; text-align:right; white-space:nowrap; }
  .muted { color:var(--tm); }
  .detail { padding:8px 0 14px; }
  .signal-note { font-size:12px; color:var(--tm); line-height:1.6; margin:18px 0 0; padding-top:14px; border-top:1px solid var(--border); max-width:80ch; }
</style>
