<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate, formatDateLong, todayISO, shiftISO, weekdayIndex } from '$lib/formatDate';

  let today = $state(todayISO());
  let selectedDate = $state(today);
  let saved = $state(false);

  // Defaults from Settings (full work day hours + which weekdays are work days).
  let workHours = $state(7.5);
  let workDays = $state<number[]>([1, 2, 3, 4, 5]);

  let rosteredHours = $state(7.5);
  let sickLeaveHours = $state(0);
  let officeHours = $state(7.5);
  let wfhHours = $state(0);

  let logs = $state<any[]>([]);

  onMount(async () => {
    try {
      const p: any = await invoke('get_app_prefs');
      if (p) {
        workHours = p.work_hours ?? 7.5;
        if (Array.isArray(p.work_days) && p.work_days.length) workDays = p.work_days;
      }
    } catch {}
    await loadDate(selectedDate);
    await loadMonthly();
  });

  async function loadDate(date: string) {
    // Prefill with the default for this weekday so a typical day is one click to
    // save; a public holiday or odd day can just be edited before saving.
    const isWorkDay = workDays.includes(weekdayIndex(date));
    rosteredHours = isWorkDay ? workHours : 0;
    officeHours = isWorkDay ? workHours : 0;
    wfhHours = 0;
    sickLeaveHours = 0;
    try {
      const existing: any = await invoke('get_daily_log', { date });
      if (existing) {
        if (existing.rostered_hours != null) rosteredHours = existing.rostered_hours;
        if (existing.sick_leave_hours != null) sickLeaveHours = existing.sick_leave_hours;
        if (existing.office_hours != null) officeHours = existing.office_hours;
        if (existing.wfh_hours != null) wfhHours = existing.wfh_hours;
      }
    } catch {}
  }

  function prevDay() {
    selectedDate = shiftISO(selectedDate, -1);
    loadDate(selectedDate);
  }

  function nextDay() {
    selectedDate = shiftISO(selectedDate, 1);
    loadDate(selectedDate);
  }

  async function save() {
    await invoke('upsert_daily_log', {
      log: {
        log_date: selectedDate,
        rostered_hours: rosteredHours,
        sick_leave_hours: sickLeaveHours,
        office_hours: officeHours,
        wfh_hours: wfhHours,
      },
    });
    saved = true;
    setTimeout(() => saved = false, 2000);
    await loadMonthly();
  }

  async function loadMonthly() {
    try {
      logs = await invoke('list_daily_logs', { limit: 60, offset: 0 }) as any[];
    } catch {}
  }

  let workedToday = $derived((officeHours + wfhHours).toFixed(1));

  function getWeekStart(dateStr: string): string {
    // Week starts on Monday: Mon→0 … Sun→6.
    const mondayOffset = (weekdayIndex(dateStr) + 6) % 7;
    return shiftISO(dateStr, -mondayOffset);
  }

  let weekGroups = $derived.by(() => {
    const sorted = [...logs].sort((a, b) => a.log_date.localeCompare(b.log_date));
    const groups: { weekStart: string; logs: any[] }[] = [];
    let currentWeek = '';
    let currentGroup: any[] = [];
    for (const log of sorted) {
      const ws = getWeekStart(log.log_date);
      if (ws !== currentWeek && currentGroup.length > 0) {
        groups.push({ weekStart: currentWeek, logs: currentGroup });
        currentGroup = [];
      }
      currentWeek = ws;
      currentGroup.push(log);
    }
    if (currentGroup.length > 0) groups.push({ weekStart: currentWeek, logs: currentGroup });
    // Most recent week first; days within a week most recent first too.
    groups.reverse();
    for (const g of groups) g.logs.reverse();
    return groups;
  });

  // Collapsible weeks: the most recent (index 0) defaults open, the rest closed.
  let openWeeks = $state<Record<string, boolean>>({});
  function isWeekOpen(weekStart: string, index: number): boolean {
    return weekStart in openWeeks ? openWeeks[weekStart] : index === 0;
  }
  function toggleWeek(weekStart: string, index: number) {
    openWeeks[weekStart] = !isWeekOpen(weekStart, index);
  }

  function totalHours(field: string, src: any[]): number {
    return src.reduce((sum: number, l: any) => sum + (l[field] ?? 0), 0);
  }

  // Status is derived from the entered hours, not picked manually:
  //   not rostered            → Day Off
  //   rostered but worked none → Sick
  //   worked less than rostered → Partial
  //   worked the full roster    → Full
  function statusBadge(log: any): { label: string; cls: string } {
    const worked = (log.office_hours ?? 0) + (log.wfh_hours ?? 0);
    const rostered = log.rostered_hours ?? 0;
    if (rostered <= 0) return { label: 'Day Off', cls: 'off' };
    if (worked <= 0) return { label: 'Sick', cls: 'sick' };
    if (worked < rostered) return { label: 'Partial', cls: 'partial' };
    return { label: 'Full', cls: 'full' };
  }

  function fmt(val: number | null | undefined): string {
    if (val == null) return '—';
    return val.toFixed(1);
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Work</div>
    <div class="page-subtitle">Hours &amp; status — feeds cognitive load &amp; tracks leave</div>
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
  </div>
</div>

<div class="two-col">
  <div class="entry-card">
    <div class="card-heading">Today's entry</div>
    <p class="entry-hint">Status is worked out automatically from the hours below.</p>
    <div class="field-grid">
      <div class="text-field">
        <label for="rostered">Rostered</label>
        <div class="input-unit">
          <input id="rostered" type="number" step="0.25" min="0" bind:value={rosteredHours} />
          <span class="unit-label">h</span>
        </div>
      </div>
      <div class="text-field">
        <label for="office">Office</label>
        <div class="input-unit">
          <input id="office" type="number" step="0.25" min="0" bind:value={officeHours} />
          <span class="unit-label">h</span>
        </div>
      </div>
      <div class="text-field">
        <label for="wfh">WFH</label>
        <div class="input-unit">
          <input id="wfh" type="number" step="0.25" min="0" bind:value={wfhHours} />
          <span class="unit-label">h</span>
        </div>
      </div>
      <div class="text-field">
        <label for="sick">Sick leave</label>
        <div class="input-unit">
          <input id="sick" type="number" step="0.25" min="0" bind:value={sickLeaveHours} />
          <span class="unit-label">h</span>
        </div>
      </div>
    </div>
    <div class="worked-box">
      <div>
        <div class="worked-label">Worked today</div>
        <div class="worked-val">{workedToday}<span class="worked-unit"> h</span></div>
      </div>
      <button class="save-btn" onclick={save}>{saved ? '✓ Saved' : 'Save'}</button>
    </div>
  </div>

  <div class="table-card">
    <div class="table-header">
      <span class="card-heading">Weekly view</span>
      <span class="table-byweek">Mon–Sun</span>
    </div>
    <div class="table-grid header-row">
      <span>Date</span><span>Status</span><span style="text-align:right;">Rost.</span><span style="text-align:right;">Office</span><span style="text-align:right;">WFH</span><span style="text-align:right;">Sick</span>
    </div>
    {#each weekGroups as group, i}
      {@const open = isWeekOpen(group.weekStart, i)}
      <button class="week-label" class:open onclick={() => toggleWeek(group.weekStart, i)}>
        <svg class="week-chevron" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
        <span>Week of {formatDate(group.weekStart)}</span>
        <span class="week-label-summary">{group.logs.length} day{group.logs.length !== 1 ? 's' : ''} · {fmt(totalHours('office_hours', group.logs) + totalHours('wfh_hours', group.logs))} h</span>
      </button>
      {#if open}
        {#each group.logs as log}
          {@const sb = statusBadge(log)}
          <div class="table-grid data-row">
            <span>{formatDate(log.log_date)}</span>
            <span><span class="status-pill {sb.cls}">{sb.label}</span></span>
            <span style="text-align:right;">{fmt(log.rostered_hours)}</span>
            <span style="text-align:right;">{fmt(log.office_hours)}</span>
            <span style="text-align:right;">{fmt(log.wfh_hours)}</span>
            <span style="text-align:right;color:var(--tm);">{fmt(log.sick_leave_hours)}</span>
          </div>
        {/each}
        <div class="table-grid data-row week-total">
          <span>Week total</span><span></span>
          <span style="text-align:right;">{fmt(totalHours('rostered_hours', group.logs))}</span>
          <span style="text-align:right;">{fmt(totalHours('office_hours', group.logs))}</span>
          <span style="text-align:right;">{fmt(totalHours('wfh_hours', group.logs))}</span>
          <span style="text-align:right;color:var(--tm);">{fmt(totalHours('sick_leave_hours', group.logs))}</span>
        </div>
      {/if}
    {/each}
    <div class="table-grid data-row month-total">
      <span style="font-family:'Source Serif 4',serif;">Month to date</span><span></span>
      <span style="text-align:right;">{fmt(totalHours('rostered_hours', logs))}</span>
      <span style="text-align:right;">{fmt(totalHours('office_hours', logs))}</span>
      <span style="text-align:right;">{fmt(totalHours('wfh_hours', logs))}</span>
      <span style="text-align:right;color:var(--tm);">{fmt(totalHours('sick_leave_hours', logs))}</span>
    </div>
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

  .two-col { display:grid; grid-template-columns:1fr 1.7fr; gap:16px; align-items:start; }
  .entry-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:20px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }

  .entry-hint { font-size:12px; color:var(--tm); line-height:1.5; margin-top:-6px; }

  .field-grid { display:grid; grid-template-columns:1fr 1fr; gap:14px; }
  .text-field { display:flex; flex-direction:column; gap:7px; }
  .text-field label { font-size:12px; font-weight:700; color:var(--ts); }
  .input-unit { display:flex; align-items:center; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px 5px; }
  .input-unit input { width:100%; background:transparent; border:none; padding:8px; font-size:13.5px; color:var(--tp); font-variant-numeric:tabular-nums; }
  .unit-label { font-size:11.5px; color:var(--tm); padding-right:8px; }

  .worked-box { background:var(--inset); border-radius:14px; padding:14px 16px; display:flex; justify-content:space-between; align-items:center; }
  .worked-label { font-size:10.5px; letter-spacing:.06em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .worked-val { font-family:'Source Serif 4',serif; font-size:24px; font-weight:600; color:var(--tp); font-variant-numeric:tabular-nums; }
  .worked-unit { font-size:13px; color:var(--tm); }
  .save-btn { background:var(--accent);color:#fff;border:none;border-radius:999px;padding:10px 18px;font-size:13px;font-weight:700;cursor:pointer; }

  .table-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .table-header { display:flex; justify-content:space-between; align-items:center; padding:18px 20px 14px; }
  .table-byweek { font-size:12px; color:var(--tm); }

  .table-grid { display:grid; grid-template-columns:1.5fr 0.9fr 0.7fr 0.7fr 0.7fr 0.7fr; padding:9px 20px; align-items:center; font-variant-numeric:tabular-nums; }
  .header-row { background:var(--inset); font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--ts); border-top:1px solid var(--border); border-bottom:1px solid var(--border); }
  .data-row { font-size:12.5px; color:var(--tp); border-top:1px solid var(--border); }
  .data-row span { white-space:nowrap; }

  .week-label { display:flex; align-items:center; gap:8px; width:100%; padding:11px 20px; font-size:10.5px; font-weight:800; color:var(--tm); letter-spacing:.04em; text-transform:uppercase; background:transparent; border:none; border-top:1px solid var(--border); cursor:pointer; font-family:inherit; text-align:left; }
  .week-label:hover { background:var(--inset); color:var(--ts); }
  .week-chevron { flex-shrink:0; transition:transform .15s; }
  .week-label.open .week-chevron { transform:rotate(90deg); }
  .week-label-summary { margin-left:auto; font-weight:700; color:var(--tm); letter-spacing:0; text-transform:none; font-variant-numeric:tabular-nums; }
  .status-pill { font-size:10.5px; font-weight:700; padding:2px 8px; border-radius:999px; }
  .status-pill.full { color:var(--accent-fg);background:var(--accent-soft); }
  .status-pill.partial { color:var(--amber-fg);background:var(--amber-soft); }
  .status-pill.sick { color:var(--red-fg);background:var(--red-soft); }
  .status-pill.off { color:var(--tm);background:var(--inset); }

  .week-total { font-size:12px; font-weight:700; color:var(--ts); background:var(--inset); border-top:1px solid var(--border); }
  .month-total { font-size:13px; font-weight:800; color:var(--tp); border-top:2px solid var(--border); }
  .month-total span:first-child { font-family:'Source Serif 4',serif; }
</style>