<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/stores';
  import { formatDateFull, todayISO, shiftISO } from '$lib/formatDate';
  import { dateFromUrl, pushDate, dateHref } from '$lib/dateParam';
  import { showToast } from '$lib/stores/toast.svelte';

  // Two days side by side. The right column is the selected date; the left is the
  // day before it. Both are fully editable, and every field writes to the date
  // printed above its own column — no field edits a day other than its heading.
  //
  // The left column carries an extra band of full-day figures (steps, active
  // calories, alcohol total, resting HR) which can't be known until the day is
  // over and the watch has synced, so they're only ever filled in the morning
  // after. That's why they appear on the left and nowhere else.

  let today = $state(todayISO());
  // Held as a plain const too, so the initial column state can be built from it
  // without reading the rune (onMount's loadPair replaces both columns anyway).
  const initialDate = dateFromUrl($page.url);
  let selectedDate = $state(initialDate);

  interface Day {
    date: string;
    /** Whether this column shows the previous-day-only totals band. */
    totals: boolean;
    log: Record<string, any>;
    symptoms: string[];
    bp: any[];
    nTime: string;
    nSys: string;
    nDia: string;
    /** Snapshot of the last saved state; an edit is anything that differs. */
    baseline: string;
  }

  // A fresh day starts at 0 fatigue / 0 headache (not blank), and every other
  // field clears — so a day with no entry never shows another day's values.
  function freshLog(date: string): Record<string, any> {
    return {
      log_date: date,
      fatigue_rating: 0,
      fatigue_desc: '',
      headache_rating: 0,
      headache_desc: '',
      headache_duration_hours: null,
      other_symptoms: '',
      my_sleep_rating: null,
      phone_sleep_rating: null,
      sleep_avg: null,
      steps: null,
      activity_calories: null,
      ave_resting_hr: null,
      alcohol_std_drinks: null,
      notes: '',
    };
  }

  function newDay(date: string, totals: boolean): Day {
    return { date, totals, log: freshLog(date), symptoms: [], bp: [], nTime: '', nSys: '', nDia: '', baseline: '' };
  }

  let dayA = $state<Day>(newDay(shiftISO(initialDate, -1), true));
  let dayB = $state<Day>(newDay(initialDate, false));

  onMount(() => {
    void loadPair(selectedDate);
    // A pending debounce would be lost if the window closed mid-edit.
    window.addEventListener('beforeunload', flushSync);
  });

  // Leaving the page (nav link, app close) flushes whatever is still pending.
  // The invoke outlives the component, so it lands even though nothing awaits it.
  onDestroy(() => {
    window.removeEventListener('beforeunload', flushSync);
    void flush();
  });

  function flushSync() { void flush(); }

  async function hydrate(d: Day) {
    try {
      const existing = await invoke('get_daily_log', { date: d.date });
      if (existing) Object.assign(d.log, existing);
    } catch {}
    // Treat a missing rating as 0 (none) rather than blank.
    if (d.log.fatigue_rating == null) d.log.fatigue_rating = 0;
    if (d.log.headache_rating == null) d.log.headache_rating = 0;
    // Other symptoms are stored as a comma-separated string; show them as chips.
    d.symptoms = parseSymptoms(d.log.other_symptoms);
    try { d.bp = await invoke('get_bp_for_date', { date: d.date }); } catch { d.bp = []; }
    // Everything the autosave watches is now in place — anything that changes
    // from here is a real edit.
    d.baseline = snapshot(d);
  }

  /// Load both columns for a selected date. Stepping a day slides one column into
  /// the other, so both are re-fetched rather than shuffled.
  async function loadPair(date: string) {
    loaded = false;
    const a = newDay(shiftISO(date, -1), true);
    const b = newDay(date, false);
    await Promise.all([hydrate(a), hydrate(b)]);
    dayA = a;
    dayB = b;
    loaded = true;
  }

  // ── Autosave ──────────────────────────────────────────────────────────────
  // Every field saves itself; there is no Save button. Edits are debounced while
  // you type, and flushed immediately whenever you leave a field, change day, or
  // leave the page — so nothing typed can be lost by navigating away. Each column
  // is its own save unit, so editing one day never writes a row for the other.

  const DEBOUNCE_MS = 700;

  let saveState = $state<'clean' | 'pending' | 'saving' | 'saved' | 'error'>('clean');
  let loaded = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let inFlight: Promise<void> | null = null;
  let savedTimer: ReturnType<typeof setTimeout> | null = null;

  // Identity of one column's editable state. The $effect below reads it, so
  // touching any field re-runs the effect; comparing against the column's
  // `baseline` tells a real edit apart from hydrate() populating the form.
  function snapshot(d: Day): string {
    const l = d.log;
    return JSON.stringify([
      l.fatigue_rating, l.fatigue_desc, l.headache_rating, l.headache_desc,
      l.headache_duration_hours, l.my_sleep_rating, l.phone_sleep_rating,
      l.alcohol_std_drinks, l.notes, l.steps, l.activity_calories, l.ave_resting_hr,
      d.symptoms,
    ]);
  }

  $effect(() => {
    // Both are read unconditionally so the effect tracks both columns.
    const a = snapshot(dayA), b = snapshot(dayB);
    if (!loaded) return;
    if (a === dayA.baseline && b === dayB.baseline) return;
    scheduleSave();
  });

  function scheduleSave() {
    if (timer) clearTimeout(timer);
    saveState = 'pending';
    timer = setTimeout(() => { timer = null; void doSave(); }, DEBOUNCE_MS);
  }

  function isDirty() {
    return loaded && (snapshot(dayA) !== dayA.baseline || snapshot(dayB) !== dayB.baseline);
  }

  /// Save now rather than at the end of the debounce. Safe to call when nothing
  /// is pending.
  async function flush() {
    if (timer) { clearTimeout(timer); timer = null; }
    if (isDirty()) await doSave();
    if (inFlight) await inFlight;
  }

  /// The columns to write for one day. Only the previous-day column carries the
  /// full-day totals; on the other column those keys are absent from the map, so
  /// `patch_daily_log` leaves them alone rather than clearing them.
  function fieldsFor(d: Day): Record<string, any> {
    const l = d.log;
    // Sleep Score Avg = mean of my rating and the Samsung score; if only one is
    // present, use that. Stored so the dashboard and PEM model read one field.
    const m = l.my_sleep_rating, p = l.phone_sleep_rating;
    const sleepAvg = (m != null && p != null) ? (m + p) / 2 : (m ?? p ?? null);
    const fields: Record<string, any> = {
      fatigue_rating: l.fatigue_rating ?? null,
      fatigue_desc: l.fatigue_desc ?? '',
      headache_rating: l.headache_rating ?? null,
      headache_desc: l.headache_desc ?? '',
      headache_duration_hours: l.headache_duration_hours ?? null,
      // The chips are stored as one comma-separated field.
      other_symptoms: d.symptoms.join(', '),
      my_sleep_rating: l.my_sleep_rating ?? null,
      phone_sleep_rating: l.phone_sleep_rating ?? null,
      sleep_avg: sleepAvg,
      alcohol_std_drinks: l.alcohol_std_drinks ?? null,
      notes: l.notes ?? '',
    };
    if (d.totals) {
      fields.steps = l.steps ?? null;
      fields.activity_calories = l.activity_calories ?? null;
      fields.ave_resting_hr = l.ave_resting_hr ?? null;
    }
    return fields;
  }

  async function doSave() {
    // Serialise: a second save must not overtake the first and lose its write.
    if (inFlight) await inFlight;

    const plan = [dayA, dayB]
      .filter((d) => snapshot(d) !== d.baseline)
      .map((d) => ({ d, snap: snapshot(d), fields: fieldsFor(d) }));
    if (plan.length === 0) { saveState = 'clean'; return; }

    saveState = 'saving';
    const run = (async () => {
      for (const p of plan) {
        await invoke('patch_daily_log', { date: p.d.date, fields: p.fields });
      }
    })();
    inFlight = run;

    try {
      await run;
      // Anything edited while the save was in flight differs from `snap`, so the
      // effect schedules a follow-up save on its own.
      for (const p of plan) {
        p.d.baseline = p.snap;
        p.d.log.sleep_avg = p.fields.sleep_avg;
      }
      saveState = 'saved';
      if (savedTimer) clearTimeout(savedTimer);
      savedTimer = setTimeout(() => { if (saveState === 'saved') saveState = 'clean'; }, 2000);
    } catch (e) {
      saveState = 'error';
      showToast(`Couldn't save: ${e}`, 'error');
    } finally {
      if (inFlight === run) inFlight = null;
    }
  }

  // Leaving a field commits it immediately instead of waiting out the debounce.
  function commitOnBlur() { void flush(); }

  // Day navigation flushes first, so pending edits land on the day they were
  // typed on rather than following you to the next one.
  async function goToDate(date: string) {
    await flush();
    selectedDate = date;
    pushDate(date);
    await loadPair(date);
  }

  function prevDay() { void goToDate(shiftISO(selectedDate, -1)); }
  function nextDay() { void goToDate(shiftISO(selectedDate, 1)); }
  function goToday() { void goToDate(today); }

  /// "Today"/"Yesterday" only when they're literally true — once you navigate
  /// back, a column headed "Today" would be a lie.
  function relLabel(date: string): string | null {
    if (date === today) return 'Today';
    if (date === shiftISO(today, -1)) return 'Yesterday';
    return null;
  }

  // Split a stored other_symptoms string into individual symptom chips. Existing
  // spreadsheet entries were written comma-separated ("Irritable, malaise, sore eyes").
  function parseSymptoms(s: string | null | undefined): string[] {
    if (!s) return [];
    return s.split(',').map((x) => x.trim()).filter((x) => x.length > 0);
  }

  function addSymptom(d: Day, e: KeyboardEvent) {
    const target = e.target as HTMLInputElement;
    const val = target.value.trim();
    if (e.key === 'Enter' && val) {
      d.symptoms = [...d.symptoms, val];
      target.value = '';
    }
  }

  function removeSymptom(d: Day, i: number) {
    d.symptoms = d.symptoms.filter((_, idx) => idx !== i);
  }

  // ── Blood pressure ────────────────────────────────────────────────────────
  // Readings are their own table (several timed readings a day), so they save on
  // the button rather than through the autosave above.

  async function addReading(d: Day) {
    if (!d.nSys || !d.nDia) return;
    try {
      const nextNum = d.bp.length > 0 ? Math.max(...d.bp.map((r: any) => r.reading_num)) + 1 : 1;
      await invoke('upsert_bp', {
        bp: {
          log_date: d.date,
          reading_num: nextNum,
          time_taken: d.nTime || null,
          systolic: parseInt(d.nSys),
          diastolic: parseInt(d.nDia),
          notes: null,
        },
      });
      d.nTime = ''; d.nSys = ''; d.nDia = '';
      d.bp = await invoke('get_bp_for_date', { date: d.date });
    } catch (e) {
      showToast(`Couldn't save reading: ${e}`, 'error');
    }
  }

  async function deleteReading(d: Day, readingNum: number) {
    try {
      await invoke('delete_bp', { logDate: d.date, readingNum });
      d.bp = await invoke('get_bp_for_date', { date: d.date });
    } catch (e) {
      showToast(`Couldn't delete reading: ${e}`, 'error');
    }
  }

  function bpAvg(d: Day): { sys: number | null; dia: number | null } {
    const valid = d.bp.filter((r: any) => r.systolic != null && r.diastolic != null);
    if (valid.length === 0) return { sys: null, dia: null };
    return {
      sys: Math.round(valid.reduce((a: number, r: any) => a + r.systolic, 0) / valid.length),
      dia: Math.round(valid.reduce((a: number, r: any) => a + r.diastolic, 0) / valid.length),
    };
  }

  function tagDot(sys: number, dia: number): string {
    if (sys >= 140 || dia >= 90) return 'var(--amber)';
    if (sys < 100 || dia < 65) return 'var(--peri)';
    return 'var(--accent)';
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Daily Log</div>
    <div class="page-subtitle">Two days side by side — yesterday's totals on the left, today on the right</div>
  </div>
  <div class="header-actions">
    <div class="day-nav">
      <button class="day-arrow" onclick={prevDay} aria-label="Previous day">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 6l-6 6 6 6"/></svg>
      </button>
      <span class="day-label">{formatDateFull(selectedDate)}</span>
      <button class="day-arrow" onclick={nextDay} disabled={selectedDate === today} aria-label="Next day">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
      </button>
    </div>
    <button class="today-btn" onclick={goToday}>Today</button>
    <span class="save-status" class:is-error={saveState === 'error'}>
      {#if saveState === 'saving' || saveState === 'pending'}
        Saving…
      {:else if saveState === 'saved'}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg>
        Saved
      {:else if saveState === 'error'}
        Not saved — retrying on your next edit
      {/if}
    </span>
  </div>
</div>

{#snippet dayHeader(d: Day, col: string, jumps: boolean)}
  <div class="day-head col-{col}">
    <div class="day-head-text">
      <span class="day-head-date">{formatDateFull(d.date)}</span>
      {#if relLabel(d.date)}<span class="day-head-rel">{relLabel(d.date)}</span>{/if}
    </div>
    <!-- The selected day carries these as full-size tiles in the card below, so
         it doesn't need them up here as well. -->
    {#if jumps}
      <div class="day-jump">
        <a class="jump-btn" href={dateHref('/medication', d.date)}>Meds</a>
        <a class="jump-btn" href={dateHref('/activity', d.date)}>Activity</a>
        <a class="jump-btn" href={dateHref('/work', d.date)}>Work</a>
      </div>
    {/if}
  </div>
{/snippet}

{#snippet totalsCard(d: Day, col: string)}
  <div class="card col-{col}">
    <div class="card-heading-row">
      <span class="card-heading">Full-day totals</span>
      <span class="card-hint">known the next morning</span>
    </div>

    <div class="text-field">
      <label for="steps-{col}">Steps</label>
      <div class="input-unit">
        <input id="steps-{col}" type="number" min="0" bind:value={d.log.steps} placeholder="0" />
        <span class="unit-label">steps</span>
      </div>
    </div>

    <div class="text-field">
      <label for="calories-{col}">Active calories</label>
      <div class="input-unit">
        <input id="calories-{col}" type="number" min="0" step="1" bind:value={d.log.activity_calories} placeholder="0" />
        <span class="unit-label">kcal</span>
      </div>
    </div>

    <div class="text-field">
      <label for="resting-{col}">Resting heart rate</label>
      <div class="input-unit">
        <input id="resting-{col}" type="number" min="0" max="250" bind:value={d.log.ave_resting_hr} placeholder="—" />
        <span class="unit-label">bpm</span>
      </div>
    </div>

    <div class="text-field">
      <label for="alcohol-{col}">Alcohol</label>
      <div class="input-unit">
        <input id="alcohol-{col}" type="number" min="0" step="0.5" bind:value={d.log.alcohol_std_drinks} placeholder="0" />
        <span class="unit-label">std drinks</span>
      </div>
    </div>
  </div>
{/snippet}

{#snippet todayCard(d: Day, col: string)}
  <!-- Stretches to the totals card opposite, and the tiles absorb the slack —
       otherwise this row leaves a hole in the selected day's column. -->
  <div class="card is-stretch col-{col}">
    <div class="quick-grid">
      <a class="quick-btn" href={dateHref('/medication', d.date)}>
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="9" width="16" height="6" rx="3"/><path d="M12 9v6"/></svg>
        <span>Medication</span>
      </a>
      <a class="quick-btn" href={dateHref('/activity', d.date)}>
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h3.5l2-6 3.5 12 2.5-6H21"/></svg>
        <span>Activity</span>
      </a>
      <a class="quick-btn" href={dateHref('/work', d.date)}>
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="7" width="18" height="13" rx="2.5"/><path d="M8 7V5.5A1.5 1.5 0 0 1 9.5 4h5A1.5 1.5 0 0 1 16 5.5V7"/></svg>
        <span>Work</span>
      </a>
    </div>

    <div class="text-field">
      <label for="alcohol-{col}">Alcohol</label>
      <div class="input-unit">
        <input id="alcohol-{col}" type="number" min="0" step="0.5" bind:value={d.log.alcohol_std_drinks} placeholder="0" />
        <span class="unit-label">std drinks</span>
      </div>
    </div>

    <div class="card-note">Steps, active calories and resting heart rate aren't complete until the day ends — they fill in on the left tomorrow morning.</div>
  </div>
{/snippet}

{#snippet feelingCard(d: Day, col: string)}
  <div class="card col-{col}">
    <div class="card-heading">How you're feeling</div>

    <div class="slider-field">
      <div class="slider-header">
        <label for="fatigue-{col}">Fatigue</label>
        <span class="slider-badge">{d.log.fatigue_rating ?? '—'} / 10</span>
      </div>
      <div class="slider-track">
        <div class="slider-fill" style="width:{(d.log.fatigue_rating != null ? (d.log.fatigue_rating / 10) * 100 : 0)}%;background:var(--accent);"></div>
        <input type="range" id="fatigue-{col}" min="0" max="10" step="0.5" bind:value={d.log.fatigue_rating} class="slider-input" />
      </div>
      <div class="slider-ends"><span>None</span><span>Severe</span></div>
    </div>

    <div class="text-field">
      <label for="fatigue-desc-{col}">Fatigue description</label>
      <input id="fatigue-desc-{col}" type="text" bind:value={d.log.fatigue_desc} />
    </div>

    <div class="slider-field">
      <div class="slider-header">
        <label for="headache-{col}">Headache</label>
        <span class="slider-badge">{d.log.headache_rating ?? '—'} / 10</span>
      </div>
      <div class="slider-track">
        <div class="slider-fill" style="width:{(d.log.headache_rating != null ? (d.log.headache_rating / 10) * 100 : 0)}%;background:var(--accent);"></div>
        <input type="range" id="headache-{col}" min="0" max="10" step="0.5" bind:value={d.log.headache_rating} class="slider-input" />
      </div>
      <div class="slider-ends"><span>None</span><span>Severe</span></div>
    </div>

    <div class="text-field">
      <label for="headache-dur-{col}">Headache duration</label>
      <div class="input-unit">
        <input id="headache-dur-{col}" type="number" step="0.5" min="0" bind:value={d.log.headache_duration_hours} placeholder="0" />
        <span class="unit-label">hrs</span>
      </div>
    </div>

    <div class="text-field" aria-label="Other symptoms">
      <label for="symptom-input-{col}">Other symptoms</label>
      <div class="symptom-chips">
        {#each d.symptoms as symptom, i}
          <span class="chip">{symptom}<button class="chip-remove" onclick={() => removeSymptom(d, i)}>×</button></span>
        {/each}
        <input id="symptom-input-{col}" type="text" placeholder="+ add" class="chip-input" onkeydown={(e) => addSymptom(d, e)} />
      </div>
    </div>
  </div>
{/snippet}

{#snippet sleepCard(d: Day, col: string)}
  <div class="card col-{col}">
    <div class="card-heading-row">
      <span class="card-heading">Sleep</span>
      <a class="card-link" href={dateHref('/sleep', d.date)}>Breakdown →</a>
    </div>

    <div class="slider-field">
      <div class="slider-header">
        <label for="sleep-rating-{col}">My sleep rating</label>
        <span class="slider-badge">{d.log.my_sleep_rating ?? '—'} / 10</span>
      </div>
      <div class="slider-track">
        <div class="slider-fill" style="width:{(d.log.my_sleep_rating != null ? (d.log.my_sleep_rating / 10) * 100 : 0)}%;background:var(--accent);"></div>
        <input type="range" id="sleep-rating-{col}" min="0" max="10" step="0.5" bind:value={d.log.my_sleep_rating} class="slider-input" />
      </div>
    </div>

    <div class="text-field">
      <label for="samsung-sleep-{col}">Samsung sleep score</label>
      <div class="input-unit">
        <input id="samsung-sleep-{col}" type="number" min="0" max="10" step="0.1" bind:value={d.log.phone_sleep_rating} placeholder="—" />
        <span class="unit-label">/ 10</span>
      </div>
    </div>
  </div>
{/snippet}

{#snippet bpCard(d: Day, col: string)}
  {@const avg = bpAvg(d)}
  <div class="card col-{col}">
    <div class="bp-header">
      <div>
        <div class="card-heading">Blood pressure</div>
        <div class="card-hint">{d.bp.length} reading{d.bp.length !== 1 ? 's' : ''} · daily average</div>
      </div>
      <div class="bp-avg">
        <span class="bp-avg-num">{avg.sys ?? '---'}</span>
        <span class="bp-avg-sep">/</span>
        <span class="bp-avg-num">{avg.dia ?? '---'}</span>
      </div>
    </div>
    <div class="bp-list">
      {#each d.bp as r}
        <div class="bp-row">
          <span class="bp-time">{r.time_taken ?? '--:--'}</span>
          <span class="bp-dot" style="background:{tagDot(r.systolic, r.diastolic)};"></span>
          <span class="bp-values"><strong>{r.systolic}/{r.diastolic}</strong></span>
          <button class="bp-delete" onclick={() => deleteReading(d, r.reading_num)} aria-label="Delete reading">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
          </button>
        </div>
      {/each}
      <div class="bp-add">
        <input type="time" bind:value={d.nTime} class="bp-input time" aria-label="Reading time" />
        <input bind:value={d.nSys} placeholder="Sys" class="bp-input xs" />
        <span class="bp-slash">/</span>
        <input bind:value={d.nDia} placeholder="Dia" class="bp-input xs" />
        <button class="add-reading-btn" onclick={() => addReading(d)}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
          Add
        </button>
      </div>
    </div>
  </div>
{/snippet}

{#snippet notesCard(d: Day, col: string)}
  <div class="card col-{col}">
    <div class="card-heading">Notes</div>
    <textarea id="daily-notes-{col}" bind:value={d.log.notes} placeholder="Anything worth noting for the day" class="notes-area"></textarea>
  </div>
{/snippet}

<!-- Leaving any field commits it, so nothing waits on the debounce to survive.
     Cards are emitted in row order (left, right, left, right …) so the grid keeps
     matching fields level across the two days. -->
<div class="day-canvas">
  <!-- Two washes behind the columns, on their own grid with the same tracks, so
       each day reads as one block. Purely decorative — kept out of the card grid
       so it can't disturb the row placement. -->
  <div class="col-bg" aria-hidden="true">
    <div class="bg-past"></div>
    <div class="bg-today"></div>
  </div>

<div class="day-grid" onfocusout={commitOnBlur}>
  {@render dayHeader(dayA, 'a', true)}
  {@render dayHeader(dayB, 'b', false)}

  {@render totalsCard(dayA, 'a')}
  {@render todayCard(dayB, 'b')}

  {@render feelingCard(dayA, 'a')}
  {@render feelingCard(dayB, 'b')}

  {@render sleepCard(dayA, 'a')}
  {@render sleepCard(dayB, 'b')}

  {@render bpCard(dayA, 'a')}
  {@render bpCard(dayB, 'b')}

  {@render notesCard(dayA, 'a')}
  {@render notesCard(dayB, 'b')}
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
  .day-label { font-weight:700; font-size:13px; padding:0 10px; min-width:168px; text-align:center; letter-spacing:.01em; }
  .today-btn { background:var(--card); border:1px solid var(--border); color:var(--ts); border-radius:999px; padding:9px 14px; font-size:12.5px; font-weight:600; cursor:pointer; white-space:nowrap; }

  /* One grid, cards emitted in row order — a row is as tall as its taller card,
     so fatigue sits level with fatigue across the two days. */
  .day-canvas { position:relative; }
  .day-grid { position:relative; z-index:1; display:grid; grid-template-columns:1fr 1.12fr; gap:16px; align-items:start; }

  /* Same tracks as the card grid, so the washes land exactly under their column.
     Each bleeds well past its outer edge and only slightly inward, leaving a
     narrow gutter between the two blocks. */
  .col-bg { position:absolute; inset:0; z-index:0; pointer-events:none; display:grid; grid-template-columns:1fr 1.12fr; gap:16px; }
  .col-bg > div { border-radius:22px; }
  .bg-past { margin:-16px -4px -16px -14px; background:var(--col-past); }
  .bg-today { margin:-16px -14px -16px -4px; background:var(--col-today); }

  .card.is-stretch { align-self:stretch; }

  /* Fills the slack opposite the totals card, so the tiles grow to whatever the
     row leaves rather than the row leaving a hole. */
  .quick-grid { display:grid; grid-template-columns:repeat(3,1fr); gap:10px; flex:1; min-height:148px; }
  .quick-btn { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:11px; text-decoration:none; background:var(--inset); border:1px solid var(--border); border-radius:16px; color:var(--ts); font-size:12.5px; font-weight:700; transition:background .14s, color .14s, border-color .14s; }
  .quick-btn:hover { background:var(--accent-soft); border-color:var(--accent); color:var(--accent-fg); }

  /* Kept to one line at the narrower of the two column widths, so the two day
     headings stay the same height as each other. */
  .day-head { display:flex; align-items:center; justify-content:space-between; gap:10px; padding:0 4px 2px; }
  .day-head-text { display:flex; align-items:baseline; gap:8px; min-width:0; }
  .day-head-date { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); white-space:nowrap; }
  .day-head-rel { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--accent-fg); background:var(--accent-soft); padding:3px 8px; border-radius:999px; }
  .day-jump { display:flex; gap:5px; flex-shrink:0; }
  .jump-btn { text-decoration:none; font-size:11px; font-weight:700; color:var(--ts); background:var(--card); border:1px solid var(--border); border-radius:999px; padding:4px 9px; white-space:nowrap; }
  .jump-btn:hover { color:var(--accent-fg); border-color:var(--accent); }

  .card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:20px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:20px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-heading-row { display:flex; align-items:center; justify-content:space-between; gap:10px; }
  .card-hint { font-size:11.5px; color:var(--tm); }
  .card-link { font-size:11.5px; font-weight:700; color:var(--accent-fg); text-decoration:none; white-space:nowrap; }
  .card-note { font-size:11.5px; color:var(--tm); line-height:1.5; }

  .slider-field { display:flex; flex-direction:column; gap:9px; }
  .slider-header { display:flex; justify-content:space-between; align-items:center; }
  .slider-header label { font-size:13.5px; font-weight:600; color:var(--tp); }
  .slider-badge { font-family:'Source Serif 4',serif; font-size:15px; font-weight:600; color:var(--accent-fg); background:var(--accent-soft); padding:2px 11px; border-radius:999px; }
  .slider-track { position:relative; height:10px; border-radius:999px; background:var(--inset); }
  .slider-fill { position:absolute; left:0; top:0; height:100%; border-radius:999px; pointer-events:none; }
  .slider-input { position:absolute; left:0; top:-4px; width:100%; height:18px; opacity:0; cursor:pointer; z-index:2; }
  .slider-ends { display:flex; justify-content:space-between; font-size:10.5px; color:var(--tm); font-weight:600; }

  .text-field { display:flex; flex-direction:column; gap:8px; }
  .text-field label { font-size:13.5px; font-weight:600; color:var(--tp); }
  .text-field input[type="text"] { width:100%; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:11px 13px; font-size:13.5px; color:var(--tp); }

  .input-unit { display:flex; align-items:center; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:4px 6px; }
  .input-unit input { width:100%; background:transparent; border:none; padding:7px; font-size:13.5px; color:var(--tp); font-variant-numeric:tabular-nums; }
  .unit-label { font-size:12px; color:var(--tm); padding-right:8px; white-space:nowrap; }

  .symptom-chips { display:flex; gap:8px; flex-wrap:wrap; align-items:center; }
  .chip { display:inline-flex; align-items:center; gap:6px; font-size:12.5px; color:var(--accent-fg); background:var(--accent-soft); border:1px solid var(--border); padding:5px 11px; border-radius:999px; }
  .chip-remove { border:none; background:transparent; color:var(--tm); cursor:pointer; font-size:14px; padding:0; }
  .chip-input { font-size:12.5px; font-weight:600; color:var(--tm); border:1px dashed var(--border); padding:5px 11px; border-radius:999px; background:transparent; min-width:60px; flex:1; }
  .chip-input:focus { outline:none; color:var(--tp); border-color:var(--accent); }

  .notes-area { width:100%; min-height:104px; resize:vertical; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:12px 13px; font-size:13.5px; color:var(--tp); line-height:1.55; }

  .bp-header { display:flex; justify-content:space-between; align-items:flex-end; gap:10px; }
  .bp-avg { display:flex; align-items:baseline; gap:3px; }
  .bp-avg-num { font-family:'Source Serif 4',serif; font-size:26px; font-weight:600; color:var(--tp); font-variant-numeric:tabular-nums; }
  .bp-avg-sep { color:var(--ts); font-size:19px; }
  .bp-list { display:flex; flex-direction:column; border:1px solid var(--border); border-radius:14px; overflow:hidden; }
  .bp-row { display:flex; align-items:center; gap:11px; padding:9px 13px; border-bottom:1px solid var(--border); }
  .bp-time { font-size:12.5px; color:var(--ts); font-variant-numeric:tabular-nums; width:44px; font-weight:600; }
  .bp-dot { width:8px;height:8px;border-radius:50%;flex-shrink:0; }
  .bp-values { flex:1; font-size:13.5px; color:var(--tp); font-variant-numeric:tabular-nums; }
  .bp-values strong { font-weight:600; }
  .bp-delete { width:24px;height:24px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .bp-add { display:flex; align-items:center; gap:6px; padding:10px 12px; background:var(--inset); }
  .bp-input { background:var(--card); border:1px solid var(--border); border-radius:9px; padding:7px; font-size:12.5px; color:var(--tp); text-align:center; font-variant-numeric:tabular-nums; min-width:0; }
  .bp-input.time { width:auto; flex-shrink:1; }
  .bp-input.xs { width:48px; }
  .bp-slash { color:var(--tm); }
  .add-reading-btn { margin-left:auto; display:inline-flex; align-items:center; gap:5px; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:7px 13px; font-size:12.5px; font-weight:700; cursor:pointer; white-space:nowrap; }

  /* Reserves its width so the header doesn't shift as the status changes. */
  .save-status { font-size:12px; color:var(--tm); display:flex; align-items:center; gap:6px; min-width:132px; }
  .save-status.is-error { color:var(--amber-fg); }

  /* Too narrow for two days: stack, with the selected day first rather than
     interleaving the pairs. */
  @media (max-width: 1180px) {
    .day-grid { grid-template-columns:1fr; }
    .col-b { order:0; }
    .col-a { order:1; }
    /* One column at a time — nothing left to tell apart. */
    .col-bg { display:none; }
  }
</style>
