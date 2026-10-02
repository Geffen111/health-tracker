<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { dateHref } from '$lib/dateParam';
  import { formatDate, todayISO, shiftISO, fatigueBand } from '$lib/formatDate';
  import Chart from '$lib/Chart.svelte';
  import { recallView, rememberView, oneOf } from '$lib/viewState';
  import { weekly } from '$lib/stores/weekly.svelte';
  import { resolveCSSVar } from '$lib/chartTheme';
  import { theme } from '$lib/stores/theme.svelte';

  let summary = $state<any>(null);
  let todayLog = $state<any>(null);
  let dailyLoads = $state<any[]>([]);
  let logs = $state<any[]>([]);
  let bpHistory = $state<any[]>([]);
  let rolling = $state<any[]>([]);
  let monthly = $state<any[]>([]);
  // Chart settings survive leaving the page (see $lib/viewState).
  const saved = recallView<any>('dashboard');
  let monthlyMetric = $state<'steps' | 'calories'>(oneOf(saved.monthlyMetric, ['steps', 'calories'] as const, 'steps'));
  let loading = $state(true);
  // Timeline: 0 = whole history.
  const RANGES = [{ days: 30, label: '30D' }, { days: 90, label: '3M' }, { days: 180, label: '6M' }, { days: 365, label: '1Y' }, { days: 0, label: 'All' }];
  let rangeDays = $state(oneOf(saved.timelineDays, RANGES.map((r) => r.days), 90));
  let showAvg = $state(saved.showAvg === true);

  const METRICS: Record<string, { label: string; field: string; color: string; format: (v: number) => string }> = {
    // Colours chosen for maximum separation — every pair must be tellable apart
    // on the line chart (previously Fatigue/Sleep were both teal-green).
    fatigue: { label: 'Fatigue', field: 'fatigue_rating', color: 'var(--accent)', format: (v) => v.toFixed(1) },
    sleep: { label: 'Sleep score', field: 'sleep_avg', color: 'var(--sky)', format: (v) => v.toFixed(1) },
    calories: { label: 'Active calories', field: 'activity_calories', color: 'var(--amber)', format: (v) => Math.round(v).toLocaleString() },
    restingHr: { label: 'Resting HR', field: 'ave_resting_hr', color: 'var(--purple)', format: (v) => v.toFixed(0) },
    headache: { label: 'Headache', field: 'headache_rating', color: 'var(--red)', format: (v) => v.toFixed(1) },
    load: { label: 'Activity load', field: 'total_load', color: 'var(--coral)', format: (v) => v.toFixed(1) },
  };
  const METRIC_KEYS = [...Object.keys(METRICS), null];
  let metricA = $state<string | null>(saved.metricA !== undefined ? oneOf(saved.metricA, METRIC_KEYS, 'fatigue') : 'fatigue');
  let metricB = $state<string | null>(saved.metricB !== undefined ? oneOf(saved.metricB, METRIC_KEYS, 'sleep') : 'sleep');

  // Event markers under the Timeline. Medication changes come from medication_history,
  // exposures and notes from the Activity page.
  const MARKERS = [
    { key: 'medication', label: 'Medication', color: 'var(--purple)', style: 'rectRot' },
    { key: 'exposure', label: 'Exposures', color: 'var(--amber)', style: 'triangle' },
    { key: 'appointment', label: 'Appointments', color: 'var(--sky)', style: 'circle' },
    { key: 'test', label: 'Tests', color: 'var(--teal)', style: 'rect' },
    { key: 'other', label: 'Other notes', color: 'var(--ts)', style: 'circle' },
  ] as const;
  type MarkerKey = typeof MARKERS[number]['key'];
  let shownMarkers = $state<Record<string, boolean>>(
    Object.fromEntries(MARKERS.map((m) => [m.key, saved.shownMarkers?.[m.key] !== false]))
  );
  let events = $state<any[]>([]);

  onMount(async () => {
    try {
      const [s, log, loads] = await Promise.all([
        invoke<any>('get_dashboard_summary'),
        invoke<any>('get_daily_log', { date: todayISO() }),
        invoke<any[]>('get_daily_loads', {}),
      ]);
      summary = s;
      todayLog = log;
      dailyLoads = loads;
      // The whole log — the Timeline can show all of it.
      [logs, events] = await Promise.all([
        invoke<any[]>('list_daily_logs', { limit: 100000, offset: 0 }),
        invoke<any[]>('get_timeline_events'),
      ]);
      bpHistory = await invoke<any[]>('get_bp_history', { days: 7 });
      [rolling, monthly] = await Promise.all([
        invoke<any[]>('get_rolling_averages'),
        invoke<any[]>('get_monthly_activity'),
      ]);
      // Keep the years compared last time (those still in the data); otherwise
      // default to the three most recent years present.
      const ys = [...new Set(monthly.map((r: any) => r.year))].sort((a, b) => a - b);
      const kept = Array.isArray(saved.selectedYears)
        ? saved.selectedYears.filter((y: number) => ys.includes(y)).slice(0, 3)
        : [];
      selectedYears = kept.length ? kept : ys.slice(-3);
      yearsReady = true;
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
  // BP is taken sporadically; show yesterday's daily average (same morning-entry
  // convention as steps), falling back to the most recent day with readings.
  let yesterdayBp = $derived.by(() => {
    const yday = shiftISO(todayISO(), -1);
    return bpHistory.find((b: any) => b.log_date === yday)
      ?? [...bpHistory].sort((a: any, b: any) => b.log_date.localeCompare(a.log_date))[0]
      ?? null;
  });
  // The headline is today's LOGGED fatigue, not a prediction — the PEM risk model was
  // retired in migration 20240622 after measuring no better than a constant.
  let fatigueBandToday = $derived(fatigueBand(fatigue));
  let loadByDate = $derived(new Map(dailyLoads.map((d: any) => [d.log_date, d])));
  let yesterdayLoad = $derived(loadByDate.get(shiftISO(todayISO(), -1)) ?? null);

  // Trailing-window helpers for the fatigue/activity summaries.
  function windowDates(back: number, span: number): string[] {
    return Array.from({ length: span }, (_, i) => shiftISO(todayISO(), -(back + i)));
  }
  let fatigueByDate = $derived(new Map(
    logs.filter((l: any) => l.fatigue_rating != null).map((l: any) => [l.log_date, l.fatigue_rating as number])
  ));
  let last7Fatigue = $derived(windowDates(0, 7).reverse().map((d) => fatigueByDate.get(d) ?? null));
  let fatigue7 = $derived.by(() => {
    const xs = last7Fatigue.filter((v): v is number => v != null);
    return xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : null;
  });
  let daysSinceBad = $derived.by(() => {
    for (let i = 0; i < 400; i++) {
      const f = fatigueByDate.get(shiftISO(todayISO(), -i));
      if (f != null && f >= 8) return i;
    }
    return null;
  });
  let hours7 = $derived(windowDates(0, 7).reduce((s, d) => s + (loadByDate.get(d)?.total_hours ?? 0), 0));
  let hoursPrev7 = $derived(windowDates(7, 7).reduce((s, d) => s + (loadByDate.get(d)?.total_hours ?? 0), 0));

  function gaugeArc(score: number | null): { pct: number; color: string } {
    if (score == null) return { pct: 0, color: 'var(--inset)' };
    const pct = Math.min(100, (score / 10) * 100);
    const band = fatigueBand(score);
    const color = band === 'High' ? 'var(--red)' : band === 'Medium' ? 'var(--amber)' : 'var(--accent)';
    return { pct, color };
  }

  let gauge = $derived(gaugeArc(fatigue));

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

  // The visible track is a 180° semicircle (left point → right point), so the fill
  // must use the same π→2π half-circle. The old 0.75π→2.25π (270°) geometry drew a
  // floating arc that didn't sit on the track.
  let gaugeArcPath = $derived.by(() => {
    if (fatigue == null) return '';
    const r = 68, cx = 85, cy = 88;
    const startAngle = Math.PI;
    const endAngle = Math.PI * 2;
    const pct = Math.min(1, Math.max(0, fatigue / 10));
    const angle = startAngle + pct * (endAngle - startAngle);
    const sx = cx + r * Math.cos(startAngle);
    const sy = cy + r * Math.sin(startAngle);
    const ex = cx + r * Math.cos(angle);
    const ey = cy + r * Math.sin(angle);
    return `M${sx} ${sy} A${r} ${r} 0 0 1 ${ex} ${ey}`;
  });

  function num(v: number | null | undefined, dp = 1): string {
    return v == null ? '—' : v.toFixed(dp);
  }
  // Activity card: this week's logged hours against the week before. Descriptive only —
  // there is no threshold to be "over", because none was ever found in the data.
  let hoursBarPct = $derived(
    Math.min(100, (hours7 / Math.max(hours7, hoursPrev7, 1)) * 100)
  );
  let hoursDesc = $derived(
    hoursPrev7 === 0 ? 'No prior week to compare'
    : `${hours7 - hoursPrev7 >= 0 ? '+' : ''}${(hours7 - hoursPrev7).toFixed(1)}h vs the week before`
  );

  let todayStr = $derived(formatDate(todayISO()));

  function bandLabel(band: string | null): string {
    if (band === 'High') return 'A heavy day — go gently';
    if (band === 'Medium') return 'A middling day';
    if (band === 'Low') return 'A good day';
    return 'Not logged yet today';
  }

  function fieldVal(log: any, field: string): number | null {
    if (field === 'total_load') {
      return loadByDate.get(log.log_date)?.total_load ?? null;
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

  // ── Timeline ──
  // One point per calendar day through yesterday (calories, HR and load aren't complete
  // until the day ends), so gaps in the log show as gaps and markers land on real dates.
  let logByDate = $derived(new Map(logs.map((l: any) => [l.log_date, l])));
  let timelineDates = $derived.by(() => {
    const end = shiftISO(todayISO(), -1);
    const first = logs.length ? logs[logs.length - 1].log_date : end;   // logs are newest-first
    let start = rangeDays === 0 ? first : shiftISO(end, -(rangeDays - 1));
    if (start > end) start = end;
    const out: string[] = [];
    for (let d = start; d <= end; d = shiftISO(d, 1)) out.push(d);
    return out;
  });
  let chartLabels = $derived(timelineDates.map((d) => formatDate(d)));
  let chartMetricA = $derived(metricA ? METRICS[metricA] ?? null : null);
  let chartMetricB = $derived(metricB ? METRICS[metricB] ?? null : null);

  function valueOn(date: string, field: string): number | null {
    const log = logByDate.get(date);
    if (field === 'total_load') return loadByDate.get(date)?.total_load ?? (log ? 0 : null);
    return log ? fieldVal(log, field) : null;
  }
  // Mean of the 7 calendar days ending on `date` (reaching back before the visible range,
  // so the line starts with a full window). Needs 3 readings to show.
  function rolling7(date: string, field: string): number | null {
    const xs: number[] = [];
    for (let i = 0; i < 7; i++) {
      const v = valueOn(shiftISO(date, -i), field);
      if (v != null) xs.push(v);
    }
    return xs.length >= 3 ? xs.reduce((a, b) => a + b, 0) / xs.length : null;
  }
  // Chart.js can't fade a CSS variable, so resolve it and add the alpha ourselves.
  function faded(color: string, alpha: number): string {
    theme.dark;   // re-derive when the theme flips
    const c = resolveCSSVar(color);
    if (/^#[0-9a-f]{6}$/i.test(c)) return c + Math.round(alpha * 255).toString(16).padStart(2, '0');
    const m = c.match(/^rgba?\(([^)]+)\)$/);
    if (m) { const [r, g, b] = m[1].split(/[ ,\/]+/); return `rgba(${r}, ${g}, ${b}, ${alpha})`; }
    return c;
  }

  // With the 7-day average on, the daily readings fade behind it (as on Pacing).
  function signalDatasets(metric: typeof METRICS[string] | null, axis: string) {
    if (!metric) return [];
    const daily = timelineDates.map((d) => valueOn(d, metric.field));
    const dense = timelineDates.length > 120;
    if (!showAvg) {
      return [{ label: metric.label, data: daily, borderColor: metric.color, backgroundColor: metric.color,
        borderWidth: dense ? 1.25 : 1.75, pointRadius: dense ? 0 : 2, yAxisID: axis, format: metric.format }];
    }
    return [
      { label: metric.label, data: daily, borderColor: faded(metric.color, 0.35), backgroundColor: faded(metric.color, 0.35),
        borderWidth: 1, pointRadius: dense ? 0 : 1.5, yAxisID: axis, format: metric.format },
      { label: `${metric.label} · 7-day avg`, data: timelineDates.map((d) => rolling7(d, metric.field)),
        borderColor: metric.color, backgroundColor: metric.color, borderWidth: 2.5, pointRadius: 0, tension: 0.35,
        yAxisID: axis, format: metric.format },
    ];
  }
  let compareDatasets = $derived([...signalDatasets(chartMetricA, 'y'), ...signalDatasets(chartMetricB, 'y1')]);

  // Events grouped by marker key and date, limited to the ones switched on.
  function markerKey(e: any): MarkerKey {
    return e.kind === 'note' ? (['appointment', 'test'].includes(e.subtype) ? e.subtype : 'other') : e.kind;
  }
  // Headings only — the notes behind each event stay on their own pages.
  const MED_EVENT: Record<string, string> = {
    started: 'Started', ceased: 'Ceased', reactivated: 'Restarted', added: 'Added',
  };
  const NOTE_LABEL: Record<string, string> = { appointment: 'Appointment', test: 'Test', other: 'Note' };
  function dose(v: string, unit: string | null): string {
    if (!unit) return v;
    return /^(mg|mcg|g|ml)$/i.test(unit) ? `${v}${unit}` : `${v} ${unit}`;
  }
  function eventLine(e: any): string {
    if (e.kind === 'medication') {
      if (e.subtype === 'dose_changed') {
        return `${e.title} dose changed${e.old_value && e.new_value ? ` — ${dose(e.old_value, e.unit)} → ${dose(e.new_value, e.unit)}` : ''}`;
      }
      return MED_EVENT[e.subtype] ? `${MED_EVENT[e.subtype]} ${e.title}` : `${e.title} — ${e.subtype}`;
    }
    if (e.kind === 'exposure') return `Exposure: ${e.title}${e.detail ? ` (${e.detail})` : ''}`;
    return `${NOTE_LABEL[markerKey(e)]}: ${e.title}`;
  }
  // Where a marker's entry is edited: medication changes on Medication, the rest on Activity.
  function openDay(path: string, date: string) {
    void goto(dateHref(path, date));
  }
  function pointerOn(e: any, active: any[]) {
    if (e.native?.target) e.native.target.style.cursor = active.length ? 'pointer' : 'default';
  }
  let eventsByDay = $derived.by(() => {
    const byKey = new Map<string, any[]>();   // `${markerKey}|${date}`
    const byDate = new Map<string, any[]>();
    for (const e of events) {
      const k = markerKey(e);
      if (!shownMarkers[k]) continue;
      byKey.set(`${k}|${e.date}`, [...(byKey.get(`${k}|${e.date}`) ?? []), e]);
      byDate.set(e.date, [...(byDate.get(e.date) ?? []), e]);
    }
    return { byKey, byDate };
  });

  // Faint vertical lines through the plot on medication-change days, drawn by an inline
  // plugin that reads the day indexes from the chart's own options.
  let medLineIdx = $derived(
    shownMarkers.medication ? timelineDates.flatMap((d, i) => eventsByDay.byKey.has(`medication|${d}`) ? [i] : []) : []
  );
  // The event strip is a second chart; it pads itself to line its plot up with the main one.
  let plotPad = $state({ left: 40, right: 10 });
  const timelinePlugins = [{
    id: 'timelineExtras',
    afterLayout(chart: any) {
      const a = chart.chartArea;
      const left = Math.round(a.left), right = Math.round(chart.width - a.right);
      // Untracked: this runs inside the main chart's effect, which mustn't depend on it.
      untrack(() => { if (left !== plotPad.left || right !== plotPad.right) plotPad = { left, right }; });
    },
    beforeDatasetsDraw(chart: any) {
      const cfg = chart.config.options.eventLines;
      if (!cfg?.indices?.length) return;
      const { ctx, chartArea: a, scales: { x } } = chart;
      ctx.save();
      ctx.strokeStyle = cfg.color;
      ctx.lineWidth = 1;
      ctx.setLineDash([3, 3]);
      for (const i of cfg.indices) {
        const px = Math.round(x.getPixelForValue(i)) + 0.5;
        ctx.beginPath(); ctx.moveTo(px, a.top); ctx.lineTo(px, a.bottom); ctx.stroke();
      }
      ctx.restore();
    },
  }];

  let compareOptions = $derived({
    elements: { point: { hoverRadius: 4 } },
    spanGaps: true,
    animation: false,
    interaction: { mode: 'index', intersect: false },
    // Clicking a day opens its Daily Log.
    onClick: (_e: any, active: any[]) => { if (active.length) openDay('/daily', timelineDates[active[0].index]); },
    onHover: pointerOn,
    eventLines: { indices: medLineIdx, color: faded('var(--purple)', 0.45) },
    scales: {
      y: { type: 'linear', position: 'left', beginAtZero: true, grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } } },
      ...(chartMetricB ? { y1: { type: 'linear', position: 'right', beginAtZero: true, grid: { drawOnChartArea: false }, ticks: { color: 'var(--ts)', font: { size: 11 } } } } : {}),
      x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 }, maxTicksLimit: 8, maxRotation: 0 } },
    },
    plugins: {
      legend: { display: true, labels: { color: 'var(--ts)', font: { size: 11 }, boxWidth: 10, padding: 12 } },
      tooltip: {
        filter: (item: any) => item.raw != null,
        callbacks: {
          label: (ctx: any) => `${ctx.dataset.label}: ${ctx.dataset.format(ctx.raw)}`,
          // The day's events, so hovering anywhere on the chart shows what happened then.
          footer: (items: any[]) => {
            const evs = items.length ? eventsByDay.byDate.get(timelineDates[items[0].dataIndex]) : null;
            return evs ? evs.map((e: any) => '• ' + eventLine(e)) : [];
          },
        },
      },
    },
  });

  // One row per marker type that's switched on, top to bottom in MARKERS order, so no
  // type can hide under another.
  let stripMarkers = $derived(MARKERS.filter((m) => shownMarkers[m.key]));
  const STRIP_ROW = 16;
  let stripDatasets = $derived(
    stripMarkers.map((m, i) => ({
      label: m.label,
      markerKey: m.key,
      data: timelineDates.map((d) => (eventsByDay.byKey.has(`${m.key}|${d}`) ? stripMarkers.length - i : null)),
      showLine: false,
      pointStyle: m.style,
      pointRadius: 5,
      pointHoverRadius: 6,
      pointHitRadius: 6,
      borderColor: m.color,
      backgroundColor: m.color,
    }))
  );
  let stripOptions = $derived({
    animation: false,
    layout: { padding: { left: plotPad.left, right: plotPad.right, top: 6, bottom: 6 } },
    interaction: { mode: 'nearest', intersect: true },
    onClick: (_e: any, active: any[]) => {
      if (!active.length) return;
      const key = stripDatasets[active[0].datasetIndex]?.markerKey;
      openDay(key === 'medication' ? '/medication' : '/activity', timelineDates[active[0].index]);
    },
    onHover: pointerOn,
    scales: {
      x: { display: false },
      y: { display: false, min: 0.4, max: stripMarkers.length + 0.6 },
    },
    plugins: {
      legend: { display: false },
      tooltip: {
        footerFont: { size: 10, weight: 'normal', style: 'italic' },
        callbacks: {
          title: (items: any[]) => (items.length ? formatDate(timelineDates[items[0].dataIndex]) : ''),
          label: (ctx: any) => {
            const evs = eventsByDay.byKey.get(`${ctx.dataset.markerKey}|${timelineDates[ctx.dataIndex]}`) ?? [];
            return evs.map(eventLine);
          },
          footer: (items: any[]) => {
            if (!items.length) return '';
            return stripDatasets[items[0].datasetIndex]?.markerKey === 'medication' ? 'Click to open Medication' : 'Click to open Activity';
          },
        },
      },
    },
  });
  let markerCounts = $derived.by(() => {
    const first = timelineDates[0], last = timelineDates[timelineDates.length - 1];
    const counts: Record<string, number> = {};
    for (const e of events) if (e.date >= first && e.date <= last) counts[markerKey(e)] = (counts[markerKey(e)] ?? 0) + 1;
    return counts;
  });

  let sleepLogs = $derived([...logs].reverse().slice(-14));
  let sleepChartData = $derived(sleepLogs.map((l: any) => sleepScore(l)));

  // ── Rolling averages table ──
  function fmtRoll(v: number | null, dp: number): string {
    return v == null ? '—' : v.toFixed(dp);
  }

  // ── Monthly steps / calories ──
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  // 2024 blue, 2025 red, 2026 amber — same order as the source spreadsheet charts.
  const YEAR_PALETTE = ['var(--sky)', 'var(--red)', 'var(--amber)', 'var(--accent)', 'var(--purple)', 'var(--peri)'];
  let monthlyField = $derived(monthlyMetric === 'steps' ? 'steps_avg' : 'calories_avg');
  // Years present in the data (ascending). The table shows all of them; the chart
  // shows only the most recent `monthlyYearsToShow` so it stays readable.
  let monthlyYears = $derived([...new Set(monthly.map((r: any) => r.year))].sort((a, b) => a - b));
  // The chart compares a chosen set of years (max 3); defaults to the most recent 3.
  // The table still shows every year.
  let selectedYears = $state<number[]>([]);
  let yearsReady = false;   // don't save the empty pre-load list over the remembered one
  let monthlyTableOpen = $state(saved.monthlyTableOpen === true);
  $effect(() => {
    rememberView('dashboard', {
      timelineDays: rangeDays, showAvg, shownMarkers, metricA, metricB, monthlyMetric, monthlyTableOpen,
      selectedYears: yearsReady ? selectedYears : saved.selectedYears,
    });
  });
  let monthlyChartYears = $derived([...selectedYears].sort((a, b) => a - b));
  function toggleYear(y: number) {
    if (selectedYears.includes(y)) {
      selectedYears = selectedYears.filter((v) => v !== y);
    } else if (selectedYears.length < 3) {
      selectedYears = [...selectedYears, y];
    }
  }
  function monthlyCell(year: number, month: number): any {
    return monthly.find((r: any) => r.year === year && r.month === month) ?? null;
  }
  // Colour by the year's absolute position so a given year keeps its colour
  // regardless of how many years are being compared.
  function yearColor(year: number): string {
    return YEAR_PALETTE[Math.max(0, monthlyYears.indexOf(year)) % YEAR_PALETTE.length];
  }
  let monthlyDatasets = $derived(monthlyChartYears.map((y: number) => ({
    label: String(y),
    data: MONTHS.map((_, mi) => {
      const row = monthlyCell(y, mi + 1);
      return row ? (row[monthlyField] ?? null) : null;
    }),
    backgroundColor: yearColor(y),
    borderColor: yearColor(y),
  })));
  let monthlyOptions = $derived({
    scales: {
      y: { beginAtZero: true, grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } } },
      x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 11 } } },
    },
    plugins: { legend: { display: true, labels: { color: 'var(--ts)', font: { size: 11 }, boxWidth: 10, padding: 12 } } },
  });
  let monthlyHasData = $derived(monthlyDatasets.some((d: any) => d.data.some((v: number | null) => v != null)));

  // Editing a cell stores a manual override. Send BOTH metrics so persisting a row
  // for a previously-computed month doesn't blank the other metric's value.
  async function editMonthlyCell(year: number, month: number, raw: string) {
    const num = raw.trim() === '' ? null : Number(raw);
    if (num != null && Number.isNaN(num)) return;
    const row = monthlyCell(year, month);
    const stepsAvg = monthlyMetric === 'steps' ? num : (row?.steps_avg ?? null);
    const caloriesAvg = monthlyMetric === 'calories' ? num : (row?.calories_avg ?? null);
    try {
      await invoke('upsert_monthly_activity', { year, month, stepsAvg, caloriesAvg });
      monthly = await invoke<any[]>('get_monthly_activity');
    } catch (e) { console.error('Error saving monthly value:', e); }
  }
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

{#if weekly.banner && !weekly.banner.seen}
  <a href="/weekly" class="weekly-banner">
    <span class="wb-icon">📋</span>
    <span class="wb-text">
      <strong>Your weekly summary is ready</strong>
      <span>Week of {formatDate(weekly.banner.week_start)} – {formatDate(weekly.banner.week_end)}</span>
    </span>
    <span class="wb-go">Read it →</span>
  </a>
{:else if weekly.generating}
  <div class="weekly-banner is-working">
    <span class="wb-icon">📋</span>
    <span class="wb-text"><strong>Writing last week's summary…</strong><span>It will appear here when it's ready.</span></span>
  </div>
{/if}

{#if loading}
  <p class="loading-text">Loading...</p>
{:else if summary}
  <div class="top-row">
    <div class="risk-card">
      <div class="risk-gauge">
        <svg viewBox="0 0 170 104" width="170" height="104">
          <path d="M17 88 A68 68 0 0 1 153 88" fill="none" stroke="var(--inset)" stroke-width="14" stroke-linecap="round"/>
          {#if gaugeArcPath}
            <path d={gaugeArcPath} fill="none" stroke={gauge.color} stroke-width="14" stroke-linecap="round"/>
          {/if}
        </svg>
        <div class="gauge-value">{fatigue != null ? fatigue.toFixed(1) : '—'}</div>
        <div class="gauge-of">logged · of 10</div>
      </div>
      <div class="risk-info">
        <div class="risk-header">
          <div class="risk-label">Today's fatigue</div>
          <span class="risk-band" style="color:{bandColor(fatigueBandToday)};background:{bandBg(fatigueBandToday)};">
            {fatigueBandToday ?? '—'}
          </span>
        </div>
        <div class="risk-desc">{bandLabel(fatigueBandToday)}</div>
        <div class="risk-stats">
          <div class="rs-tile">
            <div class="rs-label">7-day average</div>
            <div class="rs-val">{num(fatigue7, 1)}<span class="rs-sub"> /10</span></div>
          </div>
          <div class="rs-tile">
            <div class="rs-label">Since a bad day</div>
            <div class="rs-val">{daysSinceBad ?? '—'}<span class="rs-sub"> days</span></div>
          </div>
          <div class="rs-tile">
            <div class="rs-label">Activity · yday</div>
            <div class="rs-val">{num(yesterdayLoad?.total_hours, 1)}<span class="rs-sub"> h</span></div>
          </div>
          <div class="rs-tile">
            <div class="rs-label">Sleep</div>
            <div class="rs-val">{sleep != null ? sleep.toFixed(1) : '—'}<span class="rs-sub"> /10</span></div>
          </div>
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
          <div class="mini-label">BP · yday</div>
          <div class="mini-value">{yesterdayBp?.avg_systolic != null ? `${Math.round(yesterdayBp.avg_systolic)}/${Math.round(yesterdayBp.avg_diastolic)}` : '—'}<span class="mini-unit"> mmHg</span></div>
        </div>
      </div>
    </div>
  </div>

  <div class="compare-card">
    <div class="compare-header">
      <div>
        <div class="card-title">Timeline</div>
        <div class="card-subtitle">Your signals over time, with medication changes, exposures and health notes marked underneath · click a marker or day to open it</div>
      </div>
      <div class="range-toggle">
        {#each RANGES as r}
          <button class="range-btn" class:active={rangeDays === r.days} onclick={() => rangeDays = r.days}>{r.label}</button>
        {/each}
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
      <label class="avg-toggle">
        <input type="checkbox" bind:checked={showAvg} />
        7-day average
      </label>
    </div>
    <div>
      {#if compareDatasets.length === 0}
        <div class="compare-empty" style="height:240px;">Pick a signal above to plot.</div>
      {:else}
        <Chart
          type="line"
          labels={chartLabels}
          datasets={compareDatasets}
          options={compareOptions}
          plugins={timelinePlugins}
          chartArea="240px"
        />
        {#if stripDatasets.length}
          <div class="event-strip">
            <Chart type="line" labels={chartLabels} datasets={stripDatasets} options={stripOptions} chartArea="{stripMarkers.length * STRIP_ROW + 12}px" />
          </div>
        {/if}
      {/if}
    </div>
    <div class="marker-row">
      <span class="marker-row-label">Markers</span>
      {#each MARKERS as m}
        <button class="marker-chip" class:off={!shownMarkers[m.key]} onclick={() => (shownMarkers = { ...shownMarkers, [m.key]: !shownMarkers[m.key] })}>
          <span class="marker-glyph {m.style}" style="background:{m.color};"></span>
          {m.label}
          <span class="marker-count">{markerCounts[m.key] ?? 0}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="bottom-row">
    <div class="stat-card">
      <div class="stat-label">Activity · last 7 days</div>
      <div class="stat-row">
        <span class="stat-value">{num(hours7, 1)}</span>
        <span class="stat-threshold">hours logged</span>
      </div>
      <div class="progress-bar">
        <div class="progress-fill" style="width:{hoursBarPct}%;background:var(--accent);"></div>
      </div>
      <div class="stat-desc">{hoursDesc}</div>
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
      <div class="stat-label">Fatigue · last 7 days</div>
      <div class="risk-dots">
        {#each last7Fatigue as f}
          {@const b = fatigueBand(f)}
          <span
            class="risk-dot"
            class:low={b === 'Low'}
            class:med={b === 'Medium'}
            class:high={b === 'High'}
            class:none={b === null}
            title={f != null ? `${f.toFixed(1)}/10` : 'Not logged'}
          ></span>
        {/each}
      </div>
      <div class="stat-desc">{summary.bad_days_30d > 0 ? `${summary.bad_days_30d} bad days (8+) in 30` : 'No days at 8 or worse in 30'}</div>
    </div>
  </div>

  <div class="monthly-card">
    <div class="monthly-header">
      <div>
        <div class="card-title">Monthly activity</div>
        <div class="card-subtitle">Daily-average {monthlyMetric === 'steps' ? 'steps' : 'active calories'} by month · history from the spreadsheet, Jun 2026 on from your logs</div>
      </div>
      <div class="monthly-controls">
        {#if monthlyYears.length > 1}
          <details class="year-dropdown">
            <summary class="year-summary">
              <span>{selectedYears.length ? monthlyChartYears.join(', ') : 'Select years'}</span>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg>
            </summary>
            <div class="year-menu">
              <div class="year-menu-hint">Compare up to 3 years</div>
              {#each monthlyYears as y}
                {@const checked = selectedYears.includes(y)}
                <label class="year-option" class:disabled={!checked && selectedYears.length >= 3}>
                  <input type="checkbox" {checked} disabled={!checked && selectedYears.length >= 3} onchange={() => toggleYear(y)} />
                  {y}
                </label>
              {/each}
            </div>
          </details>
        {/if}
        <div class="range-toggle">
          <button class="range-btn" class:active={monthlyMetric === 'steps'} onclick={() => monthlyMetric = 'steps'}>Steps</button>
          <button class="range-btn" class:active={monthlyMetric === 'calories'} onclick={() => monthlyMetric = 'calories'}>Calories</button>
        </div>
      </div>
    </div>
    <div style="height:260px;">
      {#if monthlyHasData}
        <Chart type="bar" labels={MONTHS} datasets={monthlyDatasets} options={monthlyOptions} chartArea="260px" />
      {:else}
        <div class="compare-empty" style="height:260px;">No monthly data yet.</div>
      {/if}
    </div>
    <button class="table-toggle" onclick={() => monthlyTableOpen = !monthlyTableOpen} aria-expanded={monthlyTableOpen}>
      <svg class="chevron" class:open={monthlyTableOpen} width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
      {monthlyTableOpen ? 'Hide' : 'Show'} monthly table
    </button>
    {#if monthlyTableOpen}
      <div class="monthly-table-wrap">
        <table class="monthly-table">
          <thead>
            <tr>
              <th>Month</th>
              {#each monthlyYears as y}<th>{y}</th>{/each}
            </tr>
          </thead>
          <tbody>
            {#each MONTHS as mName, mi}
              <tr>
                <td class="m-name">{mName}</td>
                {#each monthlyYears as y}
                  {@const cell = monthlyCell(y, mi + 1)}
                  {@const val = cell && cell[monthlyField] != null ? Math.round(cell[monthlyField]) : ''}
                  <td>
                    <input
                      class="m-input"
                      class:computed={cell?.computed}
                      type="number"
                      value={val}
                      onchange={(e) => editMonthlyCell(y, mi + 1, (e.currentTarget as HTMLInputElement).value)}
                      title={cell?.computed ? 'Computed from your daily logs — type to override' : ''}
                    />
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <div class="monthly-note">Values are daily averages. Muted cells are computed from your logged days; type a value to override.</div>
    {/if}
  </div>

  <div class="rolling-card">
    <div class="card-title">Rolling averages</div>
    <div class="rolling-table-wrap">
      <table class="rolling-table">
        <thead>
          <tr>
            <th class="metric-col">Metric</th>
            <th>Last 7d</th><th>Last 30d</th><th>Last 60d</th><th>Last 90d</th>
          </tr>
        </thead>
        <tbody>
          {#each rolling as m}
            <tr>
              <td class="metric-col">{m.label}</td>
              <td>{fmtRoll(m.d7, m.dp)}</td>
              <td>{fmtRoll(m.d30, m.dp)}</td>
              <td>{fmtRoll(m.d60, m.dp)}</td>
              <td>{fmtRoll(m.d90, m.dp)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
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
  .weekly-banner {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 16px;
    padding: 14px 20px;
    border-radius: 16px;
    background: var(--accent-soft);
    border: 1px solid var(--accent);
    color: var(--accent-fg);
    text-decoration: none;
  }
  .weekly-banner.is-working { background: var(--inset); border-color: var(--border); color: var(--ts); }
  .wb-icon { font-size: 22px; }
  .wb-text { display: flex; flex-direction: column; gap: 2px; font-size: 13px; flex: 1; }
  .wb-text strong { font-size: 14.5px; color: var(--tp); }
  .wb-go { font-size: 13px; font-weight: 700; white-space: nowrap; }
  a.weekly-banner:hover { filter: brightness(0.97); }

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
    padding: 22px 24px;
    box-shadow: var(--shadow-lg);
    display: flex;
    gap: 26px;
    align-items: center;
  }
  .risk-gauge {
    flex-shrink: 0;
    width: 170px;
    height: 104px;
    position: relative;
  }
  .gauge-value {
    position: absolute;
    left: 0;
    right: 0;
    top: 44px;
    text-align: center;
    font-family: 'Source Serif 4', serif;
    font-size: 44px;
    font-weight: 600;
    color: var(--tp);
    letter-spacing: -0.02em;
  }
  .gauge-of {
    position: absolute;
    left: 0;
    right: 0;
    top: 96px;
    text-align: center;
    font-size: 10.5px;
    color: var(--tm);
    font-weight: 600;
  }
  .risk-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 11px;
    min-width: 0;
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
    font-size: 14.5px;
    color: var(--ts);
    line-height: 1.45;
  }
  .risk-stats {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 10px;
    margin-top: 4px;
  }
  .rs-tile {
    background: var(--inset);
    border-radius: 12px;
    padding: 10px 13px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .rs-label {
    font-size: 10px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    font-weight: 800;
    color: var(--ts);
  }
  .rs-val {
    font-family: 'Source Serif 4', serif;
    font-size: 21px;
    font-weight: 600;
    color: var(--tp);
  }
  .rs-sub {
    font-size: 12px;
    color: var(--tm);
    font-family: 'Public Sans', sans-serif;
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
  .metric-picker-row { flex-wrap: wrap; }
  .avg-toggle {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    font-weight: 700;
    color: var(--ts);
    cursor: pointer;
  }
  .avg-toggle input { accent-color: var(--accent); margin: 0; }
  .event-strip {
    border-top: 1px dashed var(--border);
    margin-top: 2px;
  }
  .marker-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 7px;
  }
  .marker-row-label {
    font-size: 10.5px;
    font-weight: 800;
    letter-spacing: .05em;
    text-transform: uppercase;
    color: var(--tm);
    margin-right: 2px;
  }
  .marker-chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    font-weight: 700;
    color: var(--tp);
    background: var(--inset);
    border: 1px solid var(--border);
    padding: 5px 10px;
    border-radius: 999px;
    cursor: pointer;
    font-family: inherit;
  }
  .marker-chip.off { opacity: .45; background: transparent; }
  .marker-chip.off .marker-glyph { background: var(--tm) !important; }
  .marker-count { font-size: 11px; color: var(--tm); font-variant-numeric: tabular-nums; }
  .marker-glyph { width: 9px; height: 9px; flex-shrink: 0; }
  .marker-glyph.circle { border-radius: 50%; }
  .marker-glyph.rectRot { transform: rotate(45deg) scale(.85); }
  .marker-glyph.triangle { clip-path: polygon(50% 0, 100% 100%, 0 100%); width: 10px; height: 9px; }
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
  .risk-dot { flex: 1; }
  .risk-dot.low { background: var(--accent-soft); }
  .risk-dot.med { background: var(--amber-soft); }
  .risk-dot.high { background: var(--red-soft); }
  .risk-dot.none { background: var(--inset); }

  .monthly-card, .rolling-card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 18px 20px;
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 15px;
    margin-top: 16px;
  }
  .monthly-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    flex-wrap: wrap;
  }
  .monthly-controls { display: flex; align-items: center; gap: 10px; }
  .year-dropdown { position: relative; }
  .year-summary {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: var(--inset);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 7px 13px;
    font-size: 12px;
    font-weight: 700;
    color: var(--ts);
    cursor: pointer;
    list-style: none;
    white-space: nowrap;
  }
  .year-summary::-webkit-details-marker { display: none; }
  .year-menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 20;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: var(--shadow-lg, var(--shadow));
    padding: 8px;
    min-width: 130px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .year-menu-hint { font-size: 10.5px; color: var(--tm); padding: 2px 8px 6px; }
  .year-option {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 8px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--tp);
    cursor: pointer;
  }
  .year-option:hover { background: var(--inset); }
  .year-option.disabled { color: var(--tm); cursor: not-allowed; }
  .year-option input { accent-color: var(--accent); cursor: inherit; }
  .table-toggle {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: transparent;
    border: none;
    color: var(--ts);
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
    font-family: inherit;
    padding: 2px 0;
  }
  .chevron { transition: transform 0.15s ease; }
  .chevron.open { transform: rotate(90deg); }
  .monthly-table-wrap, .rolling-table-wrap { overflow-x: auto; }
  .monthly-table, .rolling-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  .monthly-table th, .rolling-table th {
    text-align: right;
    font-size: 10.5px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    font-weight: 800;
    color: var(--ts);
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  .monthly-table th:first-child, .rolling-table th.metric-col { text-align: left; }
  .monthly-table td, .rolling-table td {
    padding: 4px 8px;
    text-align: right;
    color: var(--tp);
    font-variant-numeric: tabular-nums;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  .rolling-table td.metric-col { text-align: left; font-weight: 600; }
  .monthly-table td.m-name { text-align: left; font-weight: 600; color: var(--ts); }
  .m-input {
    width: 74px;
    background: var(--inset);
    border: 1px solid transparent;
    border-radius: 8px;
    padding: 5px 7px;
    font-size: 12.5px;
    color: var(--tp);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .m-input:hover { border-color: var(--border); }
  .m-input:focus { outline: none; border-color: var(--accent); background: var(--card); }
  .m-input.computed { color: var(--tm); font-style: italic; }
  .monthly-note { font-size: 11px; color: var(--tm); }
</style>
