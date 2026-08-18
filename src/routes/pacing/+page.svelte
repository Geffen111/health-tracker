<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate, formatDateShort, todayISO, shiftISO, weekdayIndex, fatigueBand } from '$lib/formatDate';
  import Chart from '$lib/Chart.svelte';

  // This page replaced the PEM Model screen. It predicts nothing: the old risk score
  // and next-day fatigue estimate scored no better than a constant (see migration
  // 20240622_retire_pem_predictions.sql). Everything here is a record of what happened.

  interface ActivityRow {
    log_date: string;
    category: string;
    activity_type: string;
    energy_cost: string | null;
    hours: number;
    load: number;
  }

  let rows = $state<ActivityRow[]>([]);
  let logs = $state<any[]>([]);
  let loading = $state(true);

  // ── Controls ──
  let rangeMonths = $state(6);           // 3 / 6 / 0 = all
  let bucket = $state<'week' | 'month'>('week');
  let groupBy = $state<'category' | 'activity'>('category');

  onMount(async () => {
    try {
      [rows, logs] = await Promise.all([
        invoke<ActivityRow[]>('get_activity_history', { from: null }),
        invoke<any[]>('list_daily_logs', { limit: 500, offset: 0 }),
      ]);
    } catch (e) {
      console.error('Pacing load error:', e);
    } finally {
      loading = false;
    }
  });

  // ── Range ──
  let fromDate = $derived(rangeMonths === 0 ? '0000-01-01' : shiftISO(todayISO(), -rangeMonths * 30));
  let rangeRows = $derived(rows.filter((r) => r.log_date >= fromDate));
  // Ascending, fatigue-bearing days in range — the spine for every chart below.
  let rangeLogs = $derived(
    [...logs]
      .filter((l) => l.log_date >= fromDate && l.log_date <= todayISO())
      .sort((a, b) => a.log_date.localeCompare(b.log_date))
  );

  // ── Per-day activity rollup ──
  interface DayAgg { hours: number; load: number; high: number; top: string; topHours: number }
  let dayAgg = $derived.by(() => {
    const m = new Map<string, DayAgg>();
    for (const r of rows) {
      const d = m.get(r.log_date) ?? { hours: 0, load: 0, high: 0, top: '', topHours: 0 };
      d.hours += r.hours;
      d.load += r.load;
      if (r.energy_cost === 'High') d.high += r.hours;
      if (r.hours > d.topHours) { d.top = r.activity_type; d.topHours = r.hours; }
      m.set(r.log_date, d);
    }
    return m;
  });

  let fatigueByDate = $derived.by(() => {
    const m = new Map<string, number>();
    for (const l of logs) if (l.fatigue_rating != null) m.set(l.log_date, l.fatigue_rating);
    return m;
  });

  // ── Bucketing (Monday-start weeks, or calendar months) ──
  function weekStart(iso: string): string {
    return shiftISO(iso, -((weekdayIndex(iso) + 6) % 7)); // 0=Sun → 6 days back
  }
  function bucketKey(iso: string): string {
    return bucket === 'week' ? weekStart(iso) : iso.slice(0, 7) + '-01';
  }
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  function bucketLabel(key: string): string {
    if (bucket === 'week') return formatDateShort(key);
    const [y, m] = key.split('-');
    return `${MONTHS[Number(m) - 1]} ${y.slice(2)}`;
  }

  let bucketKeys = $derived(
    [...new Set(rangeRows.map((r) => bucketKey(r.log_date)))].sort()
  );

  function seriesName(r: ActivityRow): string {
    return groupBy === 'category' ? r.category : r.activity_type;
  }

  // Every activity present in range, ranked by exertion load — not hours. Ranking by
  // hours buries the things worth watching: Phone and Smart Home dwarf Yard Work on the
  // clock, but a High-energy physical hour carries ~10x the load of a Low-energy screen
  // hour, so load puts the demanding activities at the top of the list.
  let activityRanking = $derived.by(() => {
    const totals = new Map<string, number>();
    for (const r of rangeRows) totals.set(r.activity_type, (totals.get(r.activity_type) ?? 0) + r.load);
    return [...totals.entries()].sort((a, b) => b[1] - a[1]).map(([n]) => n);
  });

  // Picked activities isolate a few series (e.g. just Yard Work and Walking). Empty =
  // the default top 9 by load, with the tail folded into "Other".
  let pickedActivities = $state<string[]>([]);
  function togglePick(name: string) {
    pickedActivities = pickedActivities.includes(name)
      ? pickedActivities.filter((n) => n !== name)
      : [...pickedActivities, name];
  }

  // Categories are few enough to show whole; activities are capped so the legend stays
  // readable, unless the picker has narrowed things down already.
  let seriesNames = $derived.by(() => {
    if (groupBy === 'category') {
      const totals = new Map<string, number>();
      for (const r of rangeRows) totals.set(r.category, (totals.get(r.category) ?? 0) + r.hours);
      return [...totals.entries()].sort((a, b) => b[1] - a[1]).map(([n]) => n);
    }
    if (pickedActivities.length) return activityRanking.filter((n) => pickedActivities.includes(n));
    if (activityRanking.length <= 10) return activityRanking;
    return [...activityRanking.slice(0, 9), 'Other'];
  });

  // With a hand-picked set, everything else is dropped rather than lumped into "Other" —
  // the point of picking is to see those activities on their own.
  let foldRest = $derived(groupBy === 'category' || pickedActivities.length === 0);

  const PALETTE = ['var(--accent)', 'var(--amber)', 'var(--sky)', 'var(--purple)', 'var(--lime)',
                   'var(--coral)', 'var(--peri)', 'var(--pink)', 'var(--teal)', 'var(--red)'];

  // hours[seriesName][bucketKey]
  let bucketedHours = $derived.by(() => {
    const keep = new Set(seriesNames);
    const m = new Map<string, Map<string, number>>();
    for (const r of rangeRows) {
      let name = seriesName(r);
      if (!keep.has(name)) {
        if (!foldRest) continue;
        name = 'Other';
      }
      const inner = m.get(name) ?? new Map<string, number>();
      const k = bucketKey(r.log_date);
      inner.set(k, (inner.get(k) ?? 0) + r.hours);
      m.set(name, inner);
    }
    return m;
  });

  // Average fatigue per bucket, overlaid so a heavy week can be read against how it felt.
  let bucketFatigue = $derived.by(() =>
    bucketKeys.map((k) => {
      const vals: number[] = [];
      for (const [date, f] of fatigueByDate) if (date >= fromDate && bucketKey(date) === k) vals.push(f);
      return vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : null;
    })
  );

  let activityDatasets = $derived([
    ...seriesNames.map((name, i) => ({
      label: name,
      data: bucketKeys.map((k) => bucketedHours.get(name)?.get(k) ?? 0),
      backgroundColor: PALETTE[i % PALETTE.length],
      borderColor: PALETTE[i % PALETTE.length],
      borderWidth: 0,
      yAxisID: 'y',
      order: 1,
    })),
    {
      type: 'line',
      label: 'Avg fatigue',
      data: bucketFatigue,
      borderColor: 'var(--red)',
      backgroundColor: 'var(--red)',
      borderWidth: 2.5,
      pointRadius: 2.5,
      tension: 0.3,
      spanGaps: true,
      yAxisID: 'y1',
      order: 2,
    },
  ]);

  let activityOptions = $derived({
    interaction: { mode: 'index', intersect: false },
    scales: {
      x: { stacked: true, grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 }, maxTicksLimit: 16 } },
      y: { stacked: true, beginAtZero: true, grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } },
           title: { display: true, text: 'Hours logged', color: 'var(--tm)', font: { size: 11 } } },
      y1: { position: 'right', min: 0, max: 10, grid: { drawOnChartArea: false }, ticks: { color: 'var(--ts)', font: { size: 11 } },
            title: { display: true, text: 'Fatigue', color: 'var(--tm)', font: { size: 11 } } },
    },
    plugins: {
      legend: { display: true, position: 'bottom', labels: { color: 'var(--ts)', font: { size: 11 }, boxWidth: 10, padding: 10 } },
    },
  });

  // ── Fatigue over time (daily + 7-day rolling mean) ──
  let fatigueSeries = $derived(rangeLogs.map((l) => l.fatigue_rating ?? null));
  let fatigueRolling = $derived.by(() =>
    rangeLogs.map((_, i) => {
      const win = rangeLogs.slice(Math.max(0, i - 6), i + 1)
        .map((l) => l.fatigue_rating).filter((v) => v != null) as number[];
      return win.length ? win.reduce((a, b) => a + b, 0) / win.length : null;
    })
  );
  let fatigueDatasets = $derived([
    { label: 'Daily', data: fatigueSeries, borderColor: 'var(--border)', backgroundColor: 'var(--border)',
      borderWidth: 1.5, pointRadius: 1.5, spanGaps: true },
    { label: '7-day average', data: fatigueRolling, borderColor: 'var(--accent)', backgroundColor: 'var(--accent)',
      borderWidth: 2.5, pointRadius: 0, tension: 0.35, spanGaps: true },
  ]);
  let fatigueOptions = {
    interaction: { mode: 'index' as const, intersect: false },
    scales: {
      x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 }, maxTicksLimit: 10 } },
      y: { min: 0, max: 10, grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 }, stepSize: 2 } },
    },
    plugins: { legend: { display: true, position: 'bottom' as const, labels: { color: 'var(--ts)', font: { size: 11 }, boxWidth: 10, padding: 10 } } },
  };

  // ── Headline tiles ──
  function mean(v: (number | null | undefined)[]): number | null {
    const xs = v.filter((x): x is number => x != null);
    return xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : null;
  }
  function windowDates(back: number, span: number): string[] {
    return Array.from({ length: span }, (_, i) => shiftISO(todayISO(), -(back + i)));
  }
  let fatigue7 = $derived(mean(windowDates(0, 7).map((d) => fatigueByDate.get(d))));
  let fatiguePrev7 = $derived(mean(windowDates(7, 7).map((d) => fatigueByDate.get(d))));
  let hours7 = $derived(windowDates(0, 7).reduce((s, d) => s + (dayAgg.get(d)?.hours ?? 0), 0));
  let hoursPrev7 = $derived(windowDates(7, 7).reduce((s, d) => s + (dayAgg.get(d)?.hours ?? 0), 0));
  let badDays30 = $derived(windowDates(0, 30).filter((d) => (fatigueByDate.get(d) ?? 0) >= 8).length);
  let daysSinceBad = $derived.by(() => {
    for (let i = 0; i < 400; i++) {
      const f = fatigueByDate.get(shiftISO(todayISO(), -i));
      if (f != null && f >= 8) return i;
    }
    return null;
  });

  function delta(now: number | null, before: number | null): string {
    if (now == null || before == null) return '';
    const d = now - before;
    if (Math.abs(d) < 0.05) return 'level on the week before';
    return `${d > 0 ? '+' : ''}${d.toFixed(1)} vs the week before`;
  }

  // ── Signal check ──
  // The honest version of what the old model claimed. Recomputed from the live data
  // every time the page opens, so it stays true as the log grows.
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

  interface Signal { label: string; r: number | null; n: number }
  let signals = $derived.by(() => {
    // One pair per day that has both logged activity and a fatigue score the next day.
    const pairs: { day: string; next: number }[] = [];
    for (const [date] of dayAgg) {
      const next = fatigueByDate.get(shiftISO(date, 1));
      if (next != null) pairs.push({ day: date, next });
    }
    const logByDate = new Map(logs.map((l) => [l.log_date, l]));
    const feature = (label: string, f: (day: string) => number | null | undefined): Signal => {
      const xs: number[] = [], ys: number[] = [];
      for (const p of pairs) {
        const v = f(p.day);
        if (v != null) { xs.push(v); ys.push(p.next); }
      }
      return { label, r: pearson(xs, ys), n: xs.length };
    };
    return [
      feature("That day's fatigue", (d) => fatigueByDate.get(d)),
      feature('Activity hours', (d) => dayAgg.get(d)?.hours),
      feature('Activity load', (d) => dayAgg.get(d)?.load),
      feature('High-energy hours', (d) => dayAgg.get(d)?.high),
      feature('Steps', (d) => logByDate.get(d)?.steps),
      feature('Sleep', (d) => logByDate.get(d)?.sleep_avg ?? logByDate.get(d)?.my_sleep_rating),
      feature('Work hours', (d) => (logByDate.get(d)?.office_hours ?? 0) + (logByDate.get(d)?.wfh_hours ?? 0)),
    ].sort((a, b) => Math.abs(b.r ?? 0) - Math.abs(a.r ?? 0));
  });

  function strength(r: number | null): { text: string; color: string } {
    if (r == null) return { text: 'not enough data', color: 'var(--tm)' };
    const a = Math.abs(r);
    if (a < 0.15) return { text: 'no signal', color: 'var(--tm)' };
    if (a < 0.3) return { text: 'very weak', color: 'var(--ts)' };
    if (a < 0.5) return { text: 'weak', color: 'var(--amber-fg)' };
    return { text: 'moderate', color: 'var(--accent-fg)' };
  }

  // ── Recent days table ──
  let recentDays = $derived(
    [...logs]
      .filter((l) => l.log_date <= todayISO())
      .sort((a, b) => b.log_date.localeCompare(a.log_date))
      .slice(0, 14)
  );

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
  function num(v: number | null | undefined, dp = 1): string {
    return v == null ? '—' : v.toFixed(dp);
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Pacing</div>
    <div class="page-subtitle">What you actually did, and how you actually felt — no predictions.</div>
  </div>
  <div class="range-toggle">
    <button class="range-btn" class:active={rangeMonths === 3} onclick={() => rangeMonths = 3}>3M</button>
    <button class="range-btn" class:active={rangeMonths === 6} onclick={() => rangeMonths = 6}>6M</button>
    <button class="range-btn" class:active={rangeMonths === 0} onclick={() => rangeMonths = 0}>All</button>
  </div>
</div>

{#if loading}
  <p class="loading-text">Loading...</p>
{:else}
  <div class="tile-row">
    <div class="tile">
      <div class="tile-label">Fatigue · 7-day average</div>
      <div class="tile-val">{num(fatigue7)}<span class="tile-unit"> /10</span></div>
      <div class="tile-desc">{delta(fatigue7, fatiguePrev7) || 'No comparison yet'}</div>
    </div>
    <div class="tile">
      <div class="tile-label">Activity · last 7 days</div>
      <div class="tile-val">{num(hours7)}<span class="tile-unit"> h</span></div>
      <div class="tile-desc">{hoursPrev7 > 0 ? `${(hours7 - hoursPrev7 > 0 ? '+' : '')}${(hours7 - hoursPrev7).toFixed(1)}h vs the week before` : 'No comparison yet'}</div>
    </div>
    <div class="tile">
      <div class="tile-label">Since a bad day</div>
      <div class="tile-val">{daysSinceBad ?? '—'}<span class="tile-unit"> days</span></div>
      <div class="tile-desc">A bad day is fatigue 8 or worse</div>
    </div>
    <div class="tile">
      <div class="tile-label">Bad days · last 30</div>
      <div class="tile-val">{badDays30}</div>
      <div class="tile-desc">Out of 30 days logged</div>
    </div>
  </div>

  <div class="card">
    <div class="card-head">
      <div>
        <div class="card-heading">Activity over time</div>
        <div class="card-subtitle">Hours logged per {bucket}, stacked by {groupBy === 'category' ? 'category' : 'activity'}, with average fatigue overlaid.</div>
      </div>
      <div class="controls">
        <div class="seg-control">
          <button class="seg-btn" class:active={bucket === 'week'} onclick={() => bucket = 'week'}>Weekly</button>
          <button class="seg-btn" class:active={bucket === 'month'} onclick={() => bucket = 'month'}>Monthly</button>
        </div>
        <div class="seg-control">
          <button class="seg-btn" class:active={groupBy === 'category'} onclick={() => groupBy = 'category'}>By category</button>
          <button class="seg-btn" class:active={groupBy === 'activity'} onclick={() => groupBy = 'activity'}>By activity</button>
        </div>
      </div>
    </div>
    {#if groupBy === 'activity'}
      <div class="picker">
        <span class="picker-label">
          {pickedActivities.length ? `Showing ${pickedActivities.length} selected` : 'Top 9 by exertion load'}
        </span>
        <div class="chips">
          {#each activityRanking as name}
            <button class="chip" class:on={pickedActivities.includes(name)} onclick={() => togglePick(name)}>{name}</button>
          {/each}
          {#if pickedActivities.length}
            <button class="chip clear" onclick={() => pickedActivities = []}>Clear</button>
          {/if}
        </div>
      </div>
    {/if}

    {#if bucketKeys.length === 0}
      <p class="empty-text">No activity logged in this range.</p>
    {:else}
      <div style="height:330px;">
        <Chart
          type="bar"
          labels={bucketKeys.map(bucketLabel)}
          datasets={activityDatasets}
          options={activityOptions}
          chartArea="330px"
        />
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="card-head">
      <div>
        <div class="card-heading">Fatigue over time</div>
        <div class="card-subtitle">Every logged day, with a 7-day average to show the underlying trend.</div>
      </div>
    </div>
    {#if rangeLogs.length === 0}
      <p class="empty-text">No fatigue logged in this range.</p>
    {:else}
      <div style="height:220px;">
        <Chart
          type="line"
          labels={rangeLogs.map((l) => formatDateShort(l.log_date))}
          datasets={fatigueDatasets}
          options={fatigueOptions}
          chartArea="220px"
        />
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="card-head">
      <div>
        <div class="card-heading">Signal check</div>
        <div class="card-subtitle">
          How strongly each measure has tracked the <em>next</em> day's fatigue, across your whole log.
          Recomputed every time this page opens.
        </div>
      </div>
    </div>
    <div class="signal-grid">
      {#each signals as s}
        {@const st = strength(s.r)}
        <div class="signal-row">
          <span class="signal-label">{s.label}</span>
          <div class="signal-bar-track">
            <div class="signal-bar" style="width:{Math.min(100, Math.abs(s.r ?? 0) * 200)}%;background:{st.color};"></div>
          </div>
          <span class="signal-r">{s.r == null ? '—' : (s.r > 0 ? '+' : '') + s.r.toFixed(2)}</span>
          <span class="signal-strength" style="color:{st.color};">{st.text}</span>
        </div>
      {/each}
    </div>
    <p class="signal-note">
      This replaced the old PEM risk score and predicted-fatigue gauge. Measured over 102 day-pairs,
      that model scored no better than always guessing your average (RMSE 1.88 vs 1.90), never once
      predicted a good day, and none of its exertion inputs correlated with the next day. Low activity
      tends to <em>follow</em> a bad day rather than precede one, which is why a load-to-risk formula
      couldn't work. A weak reading here is not proof that pacing doesn't matter — it means daily
      totals are too coarse to see it.
    </p>
  </div>

  <div class="card">
    <div class="card-heading" style="margin-bottom:12px;">Recent days</div>
    <div class="day-row head">
      <span class="d-date">Day</span>
      <span class="d-band">Fatigue</span>
      <span class="d-num">Hours</span>
      <span class="d-num">High-energy</span>
      <span class="d-num">Steps</span>
      <span class="d-top">Longest activity</span>
    </div>
    {#each recentDays as l}
      {@const band = fatigueBand(l.fatigue_rating)}
      {@const agg = dayAgg.get(l.log_date)}
      <div class="day-row">
        <span class="d-date">{formatDate(l.log_date)}</span>
        <span class="d-band" style="color:{bandColor(band)};background:{bandBg(band)};">
          {l.fatigue_rating != null ? l.fatigue_rating.toFixed(1) : '—'}
        </span>
        <span class="d-num">{agg ? num(agg.hours) : '—'}</span>
        <span class="d-num">{agg && agg.high > 0 ? num(agg.high) : '—'}</span>
        <span class="d-num">{l.steps != null ? Number(l.steps).toLocaleString() : '—'}</span>
        <span class="d-top">{agg?.top || '—'}</span>
      </div>
    {/each}
  </div>
{/if}

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:22px; gap:16px; flex-wrap:wrap; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }
  .loading-text { color:var(--ts); text-align:center; padding:32px; }

  .range-toggle { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px; gap:2px; flex-shrink:0; }
  .range-btn { background:transparent; border:none; border-radius:9px; padding:7px 14px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .range-btn.active { background:var(--accent); color:#fff; }

  .tile-row { display:grid; grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:14px; margin-bottom:16px; }
  .tile { background:var(--card); border:1px solid var(--border); border-radius:16px; padding:18px; box-shadow:var(--shadow); }
  .tile-label { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .tile-val { font-family:'Source Serif 4',serif; font-size:32px; font-weight:600; color:var(--tp); margin-top:4px; letter-spacing:-.02em; }
  .tile-unit { font-size:14px; color:var(--tm); font-family:'Public Sans',sans-serif; }
  .tile-desc { font-size:11.5px; color:var(--tm); margin-top:4px; }

  .card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); margin-bottom:16px; }
  .card-head { display:flex; justify-content:space-between; align-items:flex-start; gap:16px; margin-bottom:18px; flex-wrap:wrap; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:16px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; max-width:60ch; line-height:1.5; }
  .controls { display:flex; gap:8px; flex-wrap:wrap; }
  .seg-control { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px; gap:2px; flex-shrink:0; }
  .seg-btn { background:transparent; border:none; border-radius:9px; padding:7px 12px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .seg-btn.active { background:var(--accent); color:#fff; }
  .empty-text { color:var(--ts); font-size:13px; padding:12px 0; }

  .picker { margin:-4px 0 16px; }
  .picker-label { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--tm); }
  .chips { display:flex; flex-wrap:wrap; gap:6px; margin-top:8px; }
  .chip { background:var(--inset); border:1px solid var(--border); border-radius:999px; padding:5px 11px; font-size:11.5px;
          font-weight:600; color:var(--ts); cursor:pointer; font-family:inherit; }
  .chip.on { background:var(--accent); border-color:var(--accent); color:#fff; }
  .chip.clear { background:transparent; color:var(--accent-fg); font-weight:700; }

  .signal-grid { display:flex; flex-direction:column; gap:9px; }
  .signal-row { display:grid; grid-template-columns:150px 1fr 52px 92px; align-items:center; gap:12px; }
  .signal-label { font-size:12.5px; color:var(--tp); font-weight:600; }
  .signal-bar-track { height:8px; border-radius:999px; background:var(--inset); overflow:hidden; }
  .signal-bar { height:100%; border-radius:999px; min-width:2px; }
  .signal-r { font-size:12.5px; color:var(--ts); font-weight:700; font-variant-numeric:tabular-nums; text-align:right; }
  .signal-strength { font-size:11.5px; font-weight:700; }
  .signal-note { font-size:12px; color:var(--tm); line-height:1.6; margin:18px 0 0; padding-top:14px; border-top:1px solid var(--border); max-width:80ch; }

  .day-row { display:flex; align-items:center; gap:12px; padding:9px 14px; border-radius:8px; }
  .day-row.head { padding:4px 14px; }
  .day-row.head span { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--tm); background:transparent !important; }
  .d-date { font-size:13px; color:var(--ts); width:80px; flex-shrink:0; }
  .d-band { font-size:12px; padding:2px 8px; border-radius:5px; font-weight:700; width:52px; text-align:center; flex-shrink:0; font-variant-numeric:tabular-nums; }
  .d-num { font-size:13px; color:var(--tp); width:86px; text-align:right; flex-shrink:0; font-variant-numeric:tabular-nums; }
  .d-top { font-size:12.5px; color:var(--ts); flex:1; text-align:right; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
</style>
