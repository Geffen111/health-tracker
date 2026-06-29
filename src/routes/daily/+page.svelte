<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate, formatDateLong, todayISO, shiftISO } from '$lib/formatDate';

  let today = $state(todayISO());
  let selectedDate = $state(today);
  let log = $state<any>({
    log_date: today,
    fatigue_rating: null,
    fatigue_desc: '',
    headache_rating: null,
    headache_desc: '',
    headache_duration_hours: null,
    other_symptoms: '',
    my_sleep_rating: null,
    phone_sleep_rating: null,
    sleep_avg: null,
    alcohol_std_drinks: null,
    notes: '',
  });
  // Steps are a full-day synced metric — today's count is incomplete until the
  // day is over, so the daily log shows (and edits) YESTERDAY's complete total.
  let prevSteps = $state<number | null>(null);
  let prevCalories = $state<number | null>(null);
  let stepsDate = $derived(shiftISO(selectedDate, -1));
  let saved = $state(false);

  onMount(async () => loadDate(selectedDate));

  // A fresh day starts at 0 fatigue / 0 headache (not blank), and every other
  // field clears — so navigating to a day with no entry never shows the previous
  // day's values lingering.
  function freshLog(date: string) {
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
      alcohol_std_drinks: null,
      notes: '',
    };
  }

  async function loadDate(date: string) {
    Object.assign(log, freshLog(date));
    try {
      const existing = await invoke('get_daily_log', { date });
      if (existing) Object.assign(log, existing);
    } catch {}
    // Treat a missing rating as 0 (none) rather than blank.
    if (log.fatigue_rating == null) log.fatigue_rating = 0;
    if (log.headache_rating == null) log.headache_rating = 0;
    // Steps come from the previous day's row (yesterday's complete total).
    try {
      const p: any = await invoke('get_daily_log', { date: shiftISO(date, -1) });
      prevSteps = p?.steps ?? null;
      prevCalories = p?.activity_calories ?? null;
    } catch { prevSteps = null; prevCalories = null; }
  }

  async function save() {
    log.log_date = selectedDate;
    // Sleep Score Avg = mean of my rating and the Samsung score; if only one is
    // present, use that. Stored so the dashboard and PEM model read one field.
    const m = log.my_sleep_rating, p = log.phone_sleep_rating;
    log.sleep_avg = (m != null && p != null) ? (m + p) / 2 : (m ?? p ?? null);
    await invoke('upsert_daily_log', { log });
    // Steps & active calories are full-day metrics → the previous day's row.
    await invoke('upsert_daily_log', { log: { log_date: stepsDate, steps: prevSteps, activity_calories: prevCalories } });
    saved = true;
    setTimeout(() => saved = false, 2000);
  }

  function prevDay() {
    selectedDate = shiftISO(selectedDate, -1);
    loadDate(selectedDate);
  }

  function nextDay() {
    selectedDate = shiftISO(selectedDate, 1);
    loadDate(selectedDate);
  }

  function goToday() {
    selectedDate = today;
    loadDate(today);
  }

  let symptomList = $state<string[]>([]);

  function addSymptom(e: KeyboardEvent) {
    const target = e.target as HTMLInputElement;
    const val = target.value.trim();
    if (e.key === 'Enter' && val) {
      symptomList = [...symptomList, val];
      target.value = '';
    }
  }

  function removeSymptom(i: number) {
    symptomList = symptomList.filter((_, idx) => idx !== i);
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Daily Log</div>
    <div class="page-subtitle">How today felt — fatigue, symptoms, sleep &amp; intake</div>
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
    <button class="today-btn" onclick={goToday}>Today</button>
  </div>
</div>

<div class="two-col">
  <div class="left-col">
    <div class="card">
      <div class="card-heading">How you're feeling</div>

      <div class="slider-field">
        <div class="slider-header">
          <label for="fatigue">Fatigue</label>
          <span class="slider-badge">{log.fatigue_rating ?? '—'} / 10</span>
        </div>
        <div class="slider-track">
          <div class="slider-fill" style="width:{(log.fatigue_rating != null ? (log.fatigue_rating / 10) * 100 : 0)}%;background:var(--accent);"></div>
          <input type="range" id="fatigue" min="0" max="10" step="0.5" bind:value={log.fatigue_rating} class="slider-input" />
        </div>
        <div class="slider-ends"><span>None</span><span>Severe</span></div>
      </div>

      <div class="text-field">
        <label for="fatigue-desc">Fatigue description</label>
        <input id="fatigue-desc" type="text" bind:value={log.fatigue_desc} placeholder="e.g. Heavy legs by mid-afternoon" />
      </div>

      <div class="slider-field">
        <div class="slider-header">
          <label for="headache">Headache</label>
          <span class="slider-badge">{log.headache_rating ?? '—'} / 10</span>
        </div>
        <div class="slider-track">
          <div class="slider-fill" style="width:{(log.headache_rating != null ? (log.headache_rating / 10) * 100 : 0)}%;background:var(--accent);"></div>
          <input type="range" id="headache" min="0" max="10" step="0.5" bind:value={log.headache_rating} class="slider-input" />
        </div>
        <div class="slider-ends"><span>None</span><span>Severe</span></div>
      </div>

      <div class="text-field">
        <label for="headache-dur">Headache duration</label>
        <div class="input-unit">
          <input id="headache-dur" type="number" step="0.5" min="0" bind:value={log.headache_duration_hours} placeholder="0" />
          <span class="unit-label">hrs</span>
        </div>
      </div>

      <div class="text-field" aria-label="Other symptoms">
        <div class="symptom-chips">
          {#each symptomList as symptom, i}
            <span class="chip">{symptom}<button class="chip-remove" onclick={() => removeSymptom(i)}>×</button></span>
          {/each}
          <input type="text" placeholder="+ add" class="chip-input" onkeydown={addSymptom} />
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-heading-row">
        <span class="card-heading">Other daily notes</span>
        <span class="card-hint">Work notes live here too</span>
      </div>
      <textarea id="daily-notes" bind:value={log.notes} placeholder="Anything worth noting for the day" class="notes-area"></textarea>
    </div>
  </div>

  <div class="right-col">
    <div class="card">
      <div class="card-heading">Sleep &amp; body</div>

      <div class="slider-field">
        <div class="slider-header">
          <label for="sleep-rating">My sleep rating</label>
          <span class="slider-badge">{log.my_sleep_rating ?? '—'} / 10</span>
        </div>
        <div class="slider-track">
          <div class="slider-fill" style="width:{(log.my_sleep_rating != null ? (log.my_sleep_rating / 10) * 100 : 0)}%;background:var(--accent);"></div>
          <input type="range" id="sleep-rating" min="0" max="10" step="0.5" bind:value={log.my_sleep_rating} class="slider-input" />
        </div>
        <div class="watch-hint">Watch detail on the Sleep screen</div>
      </div>

      <div class="text-field">
        <label for="samsung-sleep">Samsung sleep score <span class="label-hint">· manual</span></label>
        <div class="input-unit">
          <input id="samsung-sleep" type="number" min="0" max="10" step="0.1" bind:value={log.phone_sleep_rating} placeholder="—" />
          <span class="unit-label">/ 10</span>
        </div>
      </div>

      <div class="text-field">
        <label for="steps">Steps <span class="label-hint">· yesterday {formatDate(stepsDate)}</span></label>
        <div class="input-unit">
          <input id="steps" type="number" min="0" bind:value={prevSteps} placeholder="0" />
          <span class="unit-label">steps</span>
        </div>
      </div>

      <div class="text-field">
        <label for="calories">Active calories <span class="label-hint">· yesterday {formatDate(stepsDate)}</span></label>
        <div class="input-unit">
          <input id="calories" type="number" min="0" step="1" bind:value={prevCalories} placeholder="0" />
          <span class="unit-label">kcal</span>
        </div>
      </div>

      <div class="text-field">
        <label for="alcohol">Alcohol</label>
        <div class="input-unit">
          <input id="alcohol" type="number" min="0" step="0.5" bind:value={log.alcohol_std_drinks} placeholder="0" />
          <span class="unit-label">std drinks</span>
        </div>
      </div>
    </div>

    <div class="save-row">
      {#if saved}
        <span class="save-status">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg>
          All changes saved
        </span>
      {:else}
        <span></span>
      {/if}
      <button class="save-btn" onclick={save}>Save day</button>
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
  .day-label { font-weight:700; font-size:13px; padding:0 6px; min-width:108px; text-align:center; letter-spacing:.01em; }
  .today-btn { background:var(--card); border:1px solid var(--border); color:var(--ts); border-radius:999px; padding:9px 14px; font-size:12.5px; font-weight:600; cursor:pointer; white-space:nowrap; }

  .two-col { display:grid; grid-template-columns:1.45fr 1fr; gap:16px; align-items:start; }
  .left-col, .right-col { display:flex; flex-direction:column; gap:16px; }

  .card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:22px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-heading-row { display:flex; align-items:center; justify-content:space-between; }
  .card-hint { font-size:11.5px; color:var(--tm); }

  .slider-field { display:flex; flex-direction:column; gap:9px; }
  .slider-header { display:flex; justify-content:space-between; align-items:center; }
  .slider-header label { font-size:13.5px; font-weight:600; color:var(--tp); }
  .slider-badge { font-family:'Source Serif 4',serif; font-size:15px; font-weight:600; color:var(--accent-fg); background:var(--accent-soft); padding:2px 11px; border-radius:999px; }
  .slider-track { position:relative; height:10px; border-radius:999px; background:var(--inset); }
  .slider-fill { position:absolute; left:0; top:0; height:100%; border-radius:999px; pointer-events:none; }
  .slider-input { position:absolute; left:0; top:-4px; width:100%; height:18px; opacity:0; cursor:pointer; z-index:2; }
  .slider-ends { display:flex; justify-content:space-between; font-size:10.5px; color:var(--tm); font-weight:600; }
  .watch-hint { font-size:11.5px; color:var(--tm); }

  .text-field { display:flex; flex-direction:column; gap:8px; }
  .text-field label { font-size:13.5px; font-weight:600; color:var(--tp); }
  .label-hint { font-weight:500; color:var(--tm); font-size:11.5px; }
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

  .save-row { display:flex; align-items:center; justify-content:space-between; gap:10px; }
  .save-status { font-size:12px; color:var(--tm); display:flex; align-items:center; gap:6px; }
  .save-btn { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:11px 22px; font-size:13.5px; font-weight:700; cursor:pointer; }
</style>
