<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate, todayISO, shiftISO, formatDateLong, weekdayIndex } from '$lib/formatDate';
  import { showToast } from '$lib/stores/toast.svelte';
  import { confirmAction } from '$lib/stores/confirm.svelte';

  let medications = $state<any[]>([]);
  let schedule = $state<any[]>([]);            // all schedule items, flat
  let history = $state<any[]>([]);
  let todayDoses = $state<any[]>([]);
  let allDoses = $state<any[]>([]);   // every logged dose, for the history tree
  let loading = $state(true);

  // Supplements (stored on the daily log) — toggled here, above today's doses.
  let supplements = $state({ multivitamin: false, vitamin_c: false });

  let today = $state(todayISO());
  let selectedDate = $state(today);

  function prevDay() { selectedDate = shiftISO(selectedDate, -1); loadAll(); }
  function nextDay() { selectedDate = shiftISO(selectedDate, 1); loadAll(); }
  function goToday() { selectedDate = today; loadAll(); }

  // Inline "log a dose" form, keyed by medication id.
  let openId = $state<number | null>(null);
  let dosing = $state<Record<number, { amount: string; time: string }>>({});

  // Add / edit medication forms.
  let showAddMed = $state(false);
  let nm = $state({ name: '', dose: '', unit: 'mg', time: '', type: 'regular' });
  let editId = $state<number | null>(null);
  let edit = $state({ name: '', dose: '', unit: 'mg', time: '', type: 'regular' });

  // Inline history edit.
  let histEditId = $state<number | null>(null);
  let histEdit = $state({ event_date: '', detail: '' });

  onMount(async () => {
    await loadAll();
    loading = false;
  });

  async function loadAll() {
    try {
      const [meds, sched, hist, doses, dlog, all] = await Promise.all([
        invoke<any[]>('list_medications'),
        invoke<any[]>('get_medication_schedule', { medicationId: null }),
        invoke<any[]>('get_medication_history', { medicationId: null }),
        invoke<any[]>('get_doses_for_date', { date: selectedDate }),
        invoke<any>('get_daily_log', { date: selectedDate }),
        invoke<any[]>('get_all_doses'),
      ]);
      medications = meds;
      schedule = sched;
      history = hist;
      todayDoses = doses;
      allDoses = all;
      supplements.multivitamin = !!dlog?.multivitamin;
      supplements.vitamin_c = !!dlog?.vitamin_c;
    } catch (e) {
      console.error('Error loading meds:', e);
    }
  }

  async function toggleSupplement(key: 'multivitamin' | 'vitamin_c') {
    supplements[key] = !supplements[key];
    await invoke('upsert_daily_log', {
      log: {
        log_date: selectedDate,
        multivitamin: supplements.multivitamin,
        vitamin_c: supplements.vitamin_c,
      },
    });
  }

  function isOccasional(m: any): boolean {
    return m.med_type === 'occasional' || m.category === 'PRN';
  }
  // Ceased meds drop off the regular/occasional lists into their own section.
  let regularMeds = $derived(medications.filter((m) => !isOccasional(m) && m.active));
  let occasionalMeds = $derived(medications.filter((m) => isOccasional(m) && m.active));
  let ceasedMeds = $derived(medications.filter((m) => !m.active));

  function slotsFor(medId: number): any[] {
    return schedule.filter((s) => s.medication_id === medId).sort((a, b) => a.sort_order - b.sort_order);
  }

  // Open the dose form pre-filled from a schedule slot (or the med's own default).
  function openDose(med: any, slot: any | null) {
    openId = med.id;
    const now = new Date();
    const nowStr = String(now.getHours()).padStart(2, '0') + ':' + String(now.getMinutes()).padStart(2, '0');
    const amount = slot?.dose_amount ?? med.default_dose;
    const time = slot?.time_of_day ?? med.default_time ?? nowStr;
    dosing[med.id] = { amount: amount != null ? String(amount) : '', time };
  }

  function cancelDose() { openId = null; }

  async function saveDose(medId: number) {
    const d = dosing[medId];
    if (!d) return;
    await invoke('upsert_dose', {
      dose: {
        medication_id: medId,
        log_date: selectedDate,
        time_taken: d.time || null,
        dose_amount: d.amount ? parseFloat(d.amount) : null,
        notes: null,
      },
    });
    openId = null;
    [todayDoses, allDoses] = await Promise.all([
      invoke<any[]>('get_doses_for_date', { date: selectedDate }),
      invoke<any[]>('get_all_doses'),
    ]);
    showToast('Dose logged');
  }

  async function deleteDose(medId: number, time: string) {
    await invoke('delete_dose', { medicationId: medId, logDate: selectedDate, timeTaken: time });
    [todayDoses, allDoses] = await Promise.all([
      invoke<any[]>('get_doses_for_date', { date: selectedDate }),
      invoke<any[]>('get_all_doses'),
    ]);
  }

  async function addMedication() {
    if (!nm.name.trim()) return;
    await invoke('create_medication', {
      name: nm.name.trim(),
      shortCode: null,
      defaultDose: nm.dose ? parseFloat(nm.dose) : null,
      doseUnit: nm.unit || 'mg',
      category: null,
      defaultTime: nm.time || null,
      medType: nm.type,
    });
    nm = { name: '', dose: '', unit: 'mg', time: '', type: 'regular' };
    showAddMed = false;
    await loadAll();
    showToast('Medication added');
  }

  function startEdit(med: any) {
    editId = med.id;
    edit = {
      name: med.name,
      dose: med.default_dose != null ? String(med.default_dose) : '',
      unit: med.dose_unit || 'mg',
      time: med.default_time || '',
      type: isOccasional(med) ? 'occasional' : 'regular',
    };
  }

  async function saveEdit(medId: number) {
    await invoke('update_medication', {
      id: medId,
      name: edit.name.trim() || null,
      defaultDose: edit.dose ? parseFloat(edit.dose) : null,
      doseUnit: edit.unit || null,
      defaultTime: edit.time || null,
      medType: edit.type,
    });
    editId = null;
    await loadAll();
    showToast('Medication updated');
  }

  async function toggleActive(med: any) {
    const banner = await invoke<string | null>('update_medication', { id: med.id, active: !med.active });
    await loadAll();
    if (banner) showToast(banner);
  }

  async function removeMed(med: any) {
    const ok = await confirmAction({
      title: `Delete ${med.name}?`,
      message: 'This removes the medication and all its logged doses. This cannot be undone.',
      confirmLabel: 'Delete',
    });
    if (!ok) return;
    await invoke('delete_medication', { id: med.id });
    await loadAll();
    showToast(`${med.name} deleted`);
  }

  // ── History editing (dates / notes on started/ceased entries) ──
  function startHistEdit(h: any) {
    histEditId = h.id;
    histEdit = { event_date: h.event_date, detail: h.detail ?? '' };
  }
  async function saveHistEdit(id: number) {
    await invoke('update_medication_history', {
      id,
      eventDate: histEdit.event_date || null,
      detail: histEdit.detail || null,
    });
    histEditId = null;
    history = await invoke('get_medication_history', { medicationId: null });
    showToast('History updated');
  }
  async function deleteHist(id: number) {
    const ok = await confirmAction({
      title: 'Delete history entry?',
      message: 'This removes this started/ceased record from the medication history.',
      confirmLabel: 'Delete',
    });
    if (!ok) return;
    await invoke('delete_medication_history', { id });
    history = await invoke('get_medication_history', { medicationId: null });
  }

  function getMedName(medId: number): string {
    return medications.find((m) => m.id === medId)?.name ?? '';
  }
  function getMedUnit(medId: number): string {
    return medications.find((m) => m.id === medId)?.dose_unit ?? 'mg';
  }

  const MED_COLORS = ['--accent', '--peri', '--amber', '--red', '--teal', '--purple', '--pink', '--lime', '--coral', '--sky'];
  function medColor(medId: number): string {
    return MED_COLORS[medId % MED_COLORS.length];
  }

  // ── Collapsible dose history: months → weeks → days, all collapsed by default ──
  const MONTH_NAMES = ['January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December'];
  function monthLabel(key: string): string {
    const [y, m] = key.split('-').map(Number);
    return `${MONTH_NAMES[m - 1]} ${y}`;
  }
  function getWeekStart(dateStr: string): string {
    // Week starts on Monday (Mon→0 … Sun→6), matching the Work page.
    const mondayOffset = (weekdayIndex(dateStr) + 6) % 7;
    return shiftISO(dateStr, -mondayOffset);
  }

  // allDoses arrives newest-first (log_date DESC, time DESC); insertion order into
  // the Maps preserves that, so months/weeks/days all read most-recent first.
  let doseHistory = $derived.by(() => {
    const months = new Map<string, Map<string, Map<string, any[]>>>();
    for (const d of allDoses) {
      const mKey = d.log_date.slice(0, 7);
      const wKey = getWeekStart(d.log_date);
      if (!months.has(mKey)) months.set(mKey, new Map());
      const weeks = months.get(mKey)!;
      if (!weeks.has(wKey)) weeks.set(wKey, new Map());
      const days = weeks.get(wKey)!;
      if (!days.has(d.log_date)) days.set(d.log_date, []);
      days.get(d.log_date)!.push(d);
    }
    const countDoses = (days: Map<string, any[]>) =>
      [...days.values()].reduce((t, ds) => t + ds.length, 0);
    return [...months.entries()].map(([mKey, weeks]) => ({
      key: mKey,
      label: monthLabel(mKey),
      count: [...weeks.values()].reduce((s, days) => s + countDoses(days), 0),
      weeks: [...weeks.entries()].map(([wKey, days]) => ({
        weekStart: wKey,
        count: countDoses(days),
        days: [...days.entries()].map(([date, doses]) => ({ date, doses })),
      })),
    }));
  });

  let openMonths = $state<Record<string, boolean>>({});
  let openWeeks = $state<Record<string, boolean>>({});
  let openDays = $state<Record<string, boolean>>({});
  function toggleOpen(map: Record<string, boolean>, key: string) { map[key] = !map[key]; }

  let eventMeta: Record<string, { label: string; style: string }> = {
    started: { label: 'Started', style: 'color:var(--accent-fg);background:var(--accent-soft);' },
    ceased: { label: 'Ceased', style: 'color:var(--red-fg);background:var(--red-soft);' },
    reactivated: { label: 'Restarted', style: 'color:var(--peri);background:var(--peri-soft);' },
    dose_changed: { label: 'Dose changed', style: 'color:var(--amber-fg);background:var(--amber-soft);' },
  };
</script>

<div class="page-header">
  <div>
    <div class="page-title">Medication</div>
    <div class="page-subtitle">Regular &amp; occasional meds, doses &amp; history</div>
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
      {#if selectedDate !== today}
        <button class="today-btn" onclick={goToday}>Today</button>
      {/if}
    </div>
    <button class="add-med-btn" onclick={() => showAddMed = !showAddMed}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      Add medication
    </button>
  </div>
</div>

{#if showAddMed}
  <div class="add-form">
    <div class="card-heading">New medication</div>
    <div class="add-form-grid">
      <div class="text-field grow">
        <label for="nm-name">Name</label>
        <input id="nm-name" bind:value={nm.name} placeholder="e.g. Sumatriptan" />
      </div>
      <div class="text-field">
        <label for="nm-dose">Dose</label>
        <input id="nm-dose" bind:value={nm.dose} placeholder="50" class="center-input" />
      </div>
      <div class="text-field">
        <label for="nm-unit">Unit</label>
        <input id="nm-unit" bind:value={nm.unit} placeholder="mg" class="center-input" />
      </div>
      <div class="text-field">
        <label for="nm-time">Usual time</label>
        <input id="nm-time" type="time" bind:value={nm.time} class="center-input" />
      </div>
      <div class="seg-field" role="radiogroup" aria-label="Medication type">
        <div class="seg-control">
          <button class="seg-btn" class:active={nm.type === 'regular'} onclick={() => nm.type = 'regular'}>Regular</button>
          <button class="seg-btn" class:active={nm.type === 'occasional'} onclick={() => nm.type = 'occasional'}>Occasional</button>
        </div>
      </div>
      <button class="save-med-btn" onclick={addMedication}>Save</button>
      <button class="cancel-med-btn" onclick={() => { showAddMed = false; nm = { name: '', dose: '', unit: 'mg', time: '', type: 'regular' }; }}>Cancel</button>
    </div>
  </div>
{/if}

{#if loading}
  <p class="loading-text">Loading...</p>
{:else}
  <div class="med-layout">
    <div class="med-list-card">
      {#each [{ label: 'Regular', meds: regularMeds }, { label: 'Occasional', meds: occasionalMeds }, { label: 'Ceased', meds: ceasedMeds }] as section}
        {#if section.label !== 'Ceased' || section.meds.length > 0}
        <div class="section-divider">{section.label}</div>
        {#if section.meds.length === 0}
          <div class="section-empty">No {section.label.toLowerCase()} medications.</div>
        {/if}
        {#each section.meds as med}
          {#if editId === med.id}
            <div class="med-edit">
              <input class="edit-name" bind:value={edit.name} placeholder="Name" />
              <input class="edit-sm" bind:value={edit.dose} placeholder="Dose" />
              <input class="edit-sm" bind:value={edit.unit} placeholder="mg" />
              <input class="edit-sm" type="time" bind:value={edit.time} />
              <div class="seg-control sm">
                <button class="seg-btn" class:active={edit.type === 'regular'} onclick={() => edit.type = 'regular'}>Reg</button>
                <button class="seg-btn" class:active={edit.type === 'occasional'} onclick={() => edit.type = 'occasional'}>Occ</button>
              </div>
              <button class="dose-save" onclick={() => saveEdit(med.id)}>Save</button>
              <button class="dose-cancel" onclick={() => editId = null}>Cancel</button>
            </div>
          {:else}
            <div class="med-row" class:dimmed={!med.active}>
              <div class="med-top">
                <div class="med-info">
                  <span class="med-name" style={med.active ? `background:var(${medColor(med.id)}-soft);` : ''}>{med.name}</span>
                  <div class="med-detail">{med.default_dose != null ? `usual ${med.default_dose}${med.dose_unit || 'mg'}` : ''}{med.default_time ? ` · ${med.default_time}` : ''}</div>
                </div>
                {#if !med.active}<span class="ceased-badge">Ceased</span>{/if}
                <button class="icon-btn" onclick={() => startEdit(med)} aria-label="Edit">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
                </button>
                <button class="icon-btn" onclick={() => toggleActive(med)} aria-label={med.active ? 'Cease' : 'Restart'} title={med.active ? 'Cease' : 'Restart'}>
                  {#if med.active}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/></svg>
                  {:else}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 4l14 8-14 8Z"/></svg>
                  {/if}
                </button>
                <button class="icon-btn danger" onclick={() => removeMed(med)} aria-label="Delete">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/></svg>
                </button>
              </div>
              {#if med.active}
                <div class="dose-buttons">
                  {#each slotsFor(med.id) as slot}
                    <button class="slot-btn" onclick={() => openDose(med, slot)}>
                      {slot.label ?? slot.time_of_day ?? 'Dose'}{slot.dose_amount != null ? ` ${slot.dose_amount}${med.dose_unit || 'mg'}` : ''}
                    </button>
                  {/each}
                  <button class="add-dose-btn" onclick={() => openDose(med, null)}>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
                    Dose
                  </button>
                </div>
              {/if}
            </div>
            {#if openId === med.id}
              <div class="dose-inline">
                <span class="dose-label">Amount</span>
                <input bind:value={dosing[med.id].amount} class="dose-input" />
                <span class="dose-unit">{med.dose_unit || 'mg'}</span>
                <span class="dose-label" style="margin-left:6px;">at</span>
                <input type="time" bind:value={dosing[med.id].time} class="dose-input wide" />
                <button class="dose-save" onclick={() => saveDose(med.id)}>Log dose</button>
                <button class="dose-cancel" onclick={cancelDose}>Cancel</button>
              </div>
            {/if}
          {/if}
        {/each}
        {/if}
      {/each}
    </div>

    <div class="right-col">
      <div class="supplements-card">
        <div class="card-heading" style="margin-bottom:12px;">Supplements</div>
        <div class="toggle-row">
          <span>Multivitamin</span>
          <button class="toggle" class:active={supplements.multivitamin} onclick={() => toggleSupplement('multivitamin')} aria-label="Toggle multivitamin">
            <span class="toggle-knob"></span>
          </button>
        </div>
        <div class="toggle-divider"></div>
        <div class="toggle-row">
          <span>Vitamin C</span>
          <button class="toggle" class:active={supplements.vitamin_c} onclick={() => toggleSupplement('vitamin_c')} aria-label="Toggle vitamin C">
            <span class="toggle-knob"></span>
          </button>
        </div>
      </div>
      <div class="doses-card">
        <div class="doses-header">
          <span class="card-heading">Doses</span>
          <span class="doses-date">{formatDate(selectedDate)}</span>
        </div>
        {#if todayDoses.length === 0}
          <p class="empty-doses">No doses logged</p>
        {:else}
          {#each todayDoses as dose}
            <div class="dose-row" style="background:var({medColor(dose.medication_id)}-soft);">
              <span class="dose-time">{dose.time_taken ?? '--:--'}</span>
              <div class="dose-med-name">{getMedName(dose.medication_id)} <span class="dose-amount">{dose.dose_amount != null ? `${dose.dose_amount} ${getMedUnit(dose.medication_id)}` : ''}</span></div>
              <button class="dose-delete" onclick={() => deleteDose(dose.medication_id, dose.time_taken)} aria-label="Delete dose">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
              </button>
            </div>
          {/each}
        {/if}
        <div class="doses-footer">{todayDoses.length} dose{todayDoses.length !== 1 ? 's' : ''} logged</div>
      </div>

      <div class="dose-hist-card">
        <div class="dose-hist-header">
          <span class="card-heading">Dose history</span>
        </div>
        {#if doseHistory.length === 0}
          <p class="empty-doses">No doses logged yet</p>
        {:else}
          {#each doseHistory as month}
            <button class="tree-row month" class:open={openMonths[month.key]} onclick={() => toggleOpen(openMonths, month.key)}>
              <svg class="tree-chevron" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
              <span class="tree-name">{month.label}</span>
              <span class="tree-count">{month.count}</span>
            </button>
            {#if openMonths[month.key]}
              {#each month.weeks as week}
                <button class="tree-row week" class:open={openWeeks[week.weekStart]} onclick={() => toggleOpen(openWeeks, week.weekStart)}>
                  <svg class="tree-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
                  <span class="tree-name">Week of {formatDate(week.weekStart)}</span>
                  <span class="tree-count">{week.count}</span>
                </button>
                {#if openWeeks[week.weekStart]}
                  {#each week.days as day}
                    <button class="tree-row day" class:open={openDays[day.date]} onclick={() => toggleOpen(openDays, day.date)}>
                      <svg class="tree-chevron" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
                      <span class="tree-name">{formatDateLong(day.date)}</span>
                      <span class="tree-count">{day.doses.length}</span>
                    </button>
                    {#if openDays[day.date]}
                      {#each day.doses as dose}
                        <div class="tree-dose" style="background:var({medColor(dose.medication_id)}-soft);">
                          <span class="dose-time">{dose.time_taken ?? '--:--'}</span>
                          <span class="tree-dose-name">{getMedName(dose.medication_id)} <span class="dose-amount">{dose.dose_amount != null ? `${dose.dose_amount} ${getMedUnit(dose.medication_id)}` : ''}</span></span>
                        </div>
                      {/each}
                    {/if}
                  {/each}
                {/if}
              {/each}
            {/if}
          {/each}
        {/if}
      </div>
    </div>
  </div>

  <div class="history-card">
    <div class="card-heading" style="margin-bottom:8px;">Medication history</div>
    {#if history.length === 0}
      <p class="empty-text">No history recorded yet.</p>
    {:else}
      {#each history as h}
        <div class="hist-row">
          <div class="hist-badge-col">
            <span class="hist-badge" style={eventMeta[h.event_type]?.style ?? ''}>{eventMeta[h.event_type]?.label ?? h.event_type}</span>
            {#if histEditId === h.id}
              <input type="date" bind:value={histEdit.event_date} class="hist-date-input" />
            {:else}
              <span class="hist-date">{formatDate(h.event_date)}</span>
            {/if}
          </div>
          <div class="hist-info">
            <div class="hist-med">{h.medication_name}</div>
            {#if histEditId === h.id}
              <input bind:value={histEdit.detail} placeholder="Add a note…" class="hist-note-input" />
              <div class="hist-actions">
                <button class="dose-save" onclick={() => saveHistEdit(h.id)}>Save</button>
                <button class="dose-cancel" onclick={() => histEditId = null}>Cancel</button>
              </div>
            {:else}
              <div class="hist-note">{h.detail ?? ''}</div>
            {/if}
          </div>
          {#if histEditId !== h.id}
            <div class="hist-row-actions">
              <button class="icon-btn" onclick={() => startHistEdit(h)} aria-label="Edit history">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
              </button>
              <button class="icon-btn danger" onclick={() => deleteHist(h.id)} aria-label="Delete history">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
              </button>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
{/if}

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:22px; gap:16px; flex-wrap:wrap; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }
  .add-med-btn { display:inline-flex;align-items:center;gap:7px;background:var(--accent);color:#fff;border:none;border-radius:999px;padding:10px 16px;font-size:13px;font-weight:700;cursor:pointer; }

  .add-form { background:var(--card);border:1px solid var(--border);border-radius:18px;padding:20px;box-shadow:var(--shadow);margin-bottom:16px;display:flex;flex-direction:column;gap:14px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .add-form-grid { display:flex; gap:12px; flex-wrap:wrap; align-items:end; }
  .text-field { display:flex; flex-direction:column; gap:6px; }
  .text-field.grow { flex:1; min-width:160px; }
  .text-field label { font-size:12px; font-weight:700; color:var(--ts); }
  .text-field input { background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:10px 12px; font-size:13.5px; color:var(--tp); }
  .center-input { text-align:center; width:96px; }
  .seg-field { display:flex; flex-direction:column; gap:6px; }
  .seg-control { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px; gap:2px; }
  .seg-control.sm { padding:2px; }
  .seg-btn { background:transparent; border:none; border-radius:9px; padding:8px 12px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .seg-btn.active { background:var(--accent); color:#fff; }
  .save-med-btn { background:var(--accent);color:#fff;border:none;border-radius:999px;padding:11px 18px;font-size:13px;font-weight:700;cursor:pointer; }
  .cancel-med-btn { background:var(--inset);color:var(--ts);border:1px solid var(--border);border-radius:999px;padding:11px 18px;font-size:13px;font-weight:700;cursor:pointer; }
  .cancel-med-btn:hover { background:var(--border); }

  .loading-text { color:var(--ts); text-align:center; padding:32px; }
  .empty-text { color:var(--ts); font-size:13px; padding:16px 0; }

  .med-layout { display:grid; grid-template-columns:1.7fr 1fr; gap:16px; align-items:start; }
  .med-list-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .section-divider { font-size:10.5px; letter-spacing:.07em; text-transform:uppercase; font-weight:800; color:var(--tm); border-top:1px solid var(--border); padding:10px 18px 6px; }
  .section-divider:first-child { border-top:none; }
  .section-empty { font-size:12.5px; color:var(--tm); padding:4px 18px 12px; }

  .med-row { display:flex; flex-direction:column; gap:7px; padding:8px 18px; border-top:1px solid var(--border); }
  .med-row.dimmed { opacity:.6; }
  .med-top { display:flex; align-items:center; gap:10px; }
  .med-info { flex:1; min-width:0; display:flex; flex-direction:row; align-items:baseline; gap:9px; flex-wrap:wrap; }
  .med-name { font-size:13.5px; font-weight:600; color:var(--tp); display:inline-block; padding:3px 9px; border-radius:7px; }
  .med-detail { font-size:11.5px; color:var(--tm); }
  .ceased-badge { font-size:10.5px; font-weight:700; color:var(--red-fg); background:var(--red-soft); padding:3px 9px; border-radius:999px; }

  .dose-buttons { display:flex; gap:6px; flex-wrap:wrap; justify-content:flex-start; padding-left:19px; }
  .slot-btn { background:var(--accent-soft); color:var(--accent-fg); border:1px solid var(--border); border-radius:999px; padding:6px 11px; font-size:11.5px; font-weight:700; cursor:pointer; white-space:nowrap; }
  .add-dose-btn { display:inline-flex;align-items:center;gap:4px;background:var(--inset);color:var(--ts);border:1px solid var(--border);border-radius:999px;padding:6px 10px;font-size:11.5px;font-weight:700;cursor:pointer;white-space:nowrap; }
  .icon-btn { width:28px;height:28px;border-radius:8px;border:1px solid var(--border);background:var(--card);color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer;flex-shrink:0; }
  .icon-btn.danger { color:var(--red-fg); }
  .icon-btn:hover { background:var(--inset); }

  .med-edit { display:flex; align-items:center; gap:8px; padding:12px 18px; border-top:1px solid var(--border); background:var(--inset); flex-wrap:wrap; }
  .edit-name { flex:1; min-width:120px; background:var(--card); border:1px solid var(--border); border-radius:9px; padding:8px; font-size:13px; color:var(--tp); }
  .edit-sm { width:70px; background:var(--card); border:1px solid var(--border); border-radius:9px; padding:8px; font-size:12.5px; color:var(--tp); text-align:center; }

  .dose-inline { display:flex;align-items:center;gap:9px;padding:12px 18px;background:var(--inset);border-top:1px solid var(--border);flex-wrap:wrap; }
  .dose-label { font-size:11.5px; color:var(--ts); font-weight:600; }
  .dose-input { width:64px;background:var(--card);border:1px solid var(--border);border-radius:9px;padding:7px;font-size:12.5px;color:var(--tp);text-align:center;font-variant-numeric:tabular-nums; }
  .dose-input.wide { width:96px; }
  .dose-unit { font-size:12px; color:var(--tm); }
  .dose-save { background:var(--accent);color:#fff;border:none;border-radius:999px;padding:8px 15px;font-size:12px;font-weight:700;cursor:pointer; }
  .dose-cancel { background:transparent;border:none;color:var(--ts);font-size:12px;font-weight:600;cursor:pointer; }

  .right-col { display:flex; flex-direction:column; gap:16px; }
  .supplements-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); padding:18px 20px; }
  .toggle-row { display:flex; align-items:center; justify-content:space-between; }
  .toggle-row span { font-size:13.5px; color:var(--tp); }
  .toggle { width:46px; height:26px; border-radius:999px; background:var(--inset); border:1px solid var(--border); position:relative; cursor:pointer; padding:0; flex-shrink:0; }
  .toggle.active { background:var(--accent); border-color:var(--accent); }
  .toggle-knob { position:absolute; top:2px; left:2px; width:20px; height:20px; border-radius:50%; background:var(--card); box-shadow:0 1px 3px rgba(0,0,0,.12); transition:left .15s; }
  .toggle.active .toggle-knob { left:22px; background:#fff; box-shadow:0 1px 3px rgba(0,0,0,.2); }
  .toggle-divider { height:1px; background:var(--border); margin:12px 0; }
  .doses-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .doses-header { display:flex; justify-content:space-between; align-items:center; padding:16px 18px 12px; }
  .doses-date { font-size:11.5px; color:var(--tm); }
  .empty-doses { color:var(--ts); text-align:center; padding:24px; font-size:13px; }
  .dose-row { display:flex; align-items:center; gap:11px; padding:11px 18px; border-top:1px solid var(--border); }
  .dose-time { font-size:12px; color:var(--ts); font-variant-numeric:tabular-nums; width:42px; font-weight:600; }
  .dose-med-name { flex:1; min-width:0; font-size:13px; color:var(--tp); }
  .dose-amount { color:var(--tm); font-size:12px; }
  .dose-delete { width:24px;height:24px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .doses-footer { padding:12px 18px; border-top:1px solid var(--border); font-size:12px; color:var(--tm); }

  .dose-hist-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .dose-hist-header { padding:16px 18px 12px; }
  .tree-row { display:flex; align-items:center; gap:8px; width:100%; background:transparent; border:none; border-top:1px solid var(--border); cursor:pointer; font-family:inherit; text-align:left; color:var(--tp); }
  .tree-row:hover { background:var(--inset); }
  .tree-chevron { flex-shrink:0; transition:transform .15s; color:var(--tm); }
  .tree-row.open > .tree-chevron { transform:rotate(90deg); }
  .tree-name { flex:1; min-width:0; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .tree-count { font-size:11px; font-weight:700; color:var(--tm); background:var(--inset); border-radius:999px; padding:2px 8px; font-variant-numeric:tabular-nums; flex-shrink:0; }
  .tree-row.month { padding:12px 18px; font-size:13px; font-weight:700; }
  .tree-row.week { padding:10px 18px 10px 30px; font-size:12px; font-weight:600; color:var(--ts); }
  .tree-row.day { padding:9px 18px 9px 44px; font-size:12px; color:var(--ts); }
  .tree-row.week .tree-count, .tree-row.day .tree-count { background:transparent; }
  .tree-dose { display:flex; align-items:center; gap:11px; padding:9px 18px 9px 44px; border-top:1px solid var(--border); }
  .tree-dose-name { flex:1; min-width:0; font-size:12.5px; color:var(--tp); }

  .history-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); margin-top:16px; display:flex; flex-direction:column; gap:4px; }
  .hist-row { display:flex; gap:14px; padding:12px 0; border-top:1px solid var(--border); align-items:flex-start; }
  .hist-badge-col { width:104px; flex-shrink:0; display:flex; flex-direction:column; gap:5px; }
  .hist-badge { font-size:10.5px; font-weight:700; padding:3px 9px; border-radius:999px; text-align:center; }
  .hist-date { font-size:11.5px; color:var(--tm); font-variant-numeric:tabular-nums; text-align:center; }
  .hist-date-input { font-size:11.5px; border:1px solid var(--border); border-radius:8px; padding:5px; background:var(--inset); color:var(--tp); }
  .hist-info { flex:1; min-width:0; display:flex; flex-direction:column; gap:6px; }
  .hist-med { font-size:13.5px; font-weight:600; color:var(--tp); }
  .hist-note { font-size:12.5px; color:var(--ts); }
  .hist-note-input { width:100%; font-size:12.5px; border:1px solid var(--border); border-radius:9px; padding:8px 10px; background:var(--inset); color:var(--tp); }
  .hist-actions { display:flex; gap:8px; }
  .hist-row-actions { display:flex; gap:6px; flex-shrink:0; }
  .header-actions { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .day-nav { display:flex; align-items:center; gap:2px; background:var(--card); border:1px solid var(--border); border-radius:999px; padding:4px; box-shadow:var(--shadow); }
  .day-arrow { width:30px;height:30px;border-radius:50%;border:none;background:transparent;color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .day-arrow:disabled { color:var(--tm); cursor:not-allowed; }
  .day-label { font-weight:700; font-size:13px; padding:0 6px; min-width:108px; text-align:center; letter-spacing:.01em; }
  .today-btn { background:var(--accent-soft);color:var(--accent-fg);border:none;border-radius:999px;padding:6px 12px;font-size:12px;font-weight:700;cursor:pointer;margin-left:4px; }
  .today-btn:hover { background:var(--accent); color:#fff; }
</style>
