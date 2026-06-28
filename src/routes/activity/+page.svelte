<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateLong, todayISO, shiftISO } from '$lib/formatDate';
  import { computeDayLoad } from '$lib/load';

  let today = $state(todayISO());
  let selectedDate = $state(today);
  let categories = $state<any[]>([]);
  let activityTypes = $state<any[]>([]);
  let entries = $state<any[]>([]);
  let activityDefaults = $state<string[]>(['Phone', 'Walking']);
  let loading = $state(true);

  // type_id -> duration string (what's shown in each row's input)
  let durations = $state<Record<number, string>>({});

  let addTypeId = $state<number | null>(null);
  let addDuration = $state('');

  onMount(async () => {
    try {
      const [cats, types, prefs] = await Promise.all([
        invoke<any[]>('list_activity_categories'),
        invoke<any[]>('list_activity_types', { categoryId: null }),
        invoke<any>('get_app_prefs'),
      ]);
      categories = cats;
      activityTypes = types;
      if (prefs?.activity_defaults?.length) activityDefaults = prefs.activity_defaults;
      await loadEntries();
    } catch (e) {
      console.error('Error loading activity data:', e);
    } finally {
      loading = false;
    }
  });

  async function loadEntries() {
    entries = await invoke('get_activities_for_date', { date: selectedDate });
    const map: Record<number, string> = {};
    for (const e of entries) map[e.activity_type_id] = String(e.duration_hours);
    durations = map;
  }

  function typeByName(name: string): any | undefined {
    return activityTypes.find((t: any) => t.name === name);
  }
  function getType(typeId: number): any | undefined {
    return activityTypes.find((t: any) => t.id === typeId);
  }
  function getCategoryName(catId: number | null | undefined): string {
    return categories.find((c: any) => c.id === catId)?.name ?? '';
  }

  // The rows shown for the day: the configured defaults first (always present,
  // ready for a time), then any other activity already logged for the day.
  let rowTypeIds = $derived.by(() => {
    const ids: number[] = [];
    for (const name of activityDefaults) {
      const t = typeByName(name);
      if (t && !ids.includes(t.id)) ids.push(t.id);
    }
    for (const e of entries) if (!ids.includes(e.activity_type_id)) ids.push(e.activity_type_id);
    return ids;
  });

  // Types available to add (not already shown as a row).
  let addableTypes = $derived(activityTypes.filter((t: any) => !rowTypeIds.includes(t.id)));

  async function saveRow(typeId: number) {
    const raw = durations[typeId];
    const v = raw == null || raw === '' ? 0 : parseFloat(raw);
    await invoke('set_activity_duration', {
      logDate: selectedDate,
      activityTypeId: typeId,
      durationHours: isNaN(v) ? 0 : v,
    });
    await loadEntries();
  }

  async function clearRow(typeId: number) {
    durations[typeId] = '';
    await saveRow(typeId);
  }

  async function addActivity() {
    if (!addTypeId) return;
    const v = parseFloat(addDuration);
    await invoke('set_activity_duration', {
      logDate: selectedDate,
      activityTypeId: addTypeId,
      durationHours: isNaN(v) ? 0 : v,
    });
    addTypeId = null;
    addDuration = '';
    await loadEntries();
  }

  function prevDay() { selectedDate = shiftISO(selectedDate, -1); loadEntries(); }
  function nextDay() { selectedDate = shiftISO(selectedDate, 1); loadEntries(); }

  let loadBuckets = $derived.by(() => {
    const { phys, cog, sens, total } = computeDayLoad(entries, activityTypes, categories);
    const scale = Math.max(total > 0 ? Math.max(phys, cog, sens) : 1, 0.001);
    const pct = (v: number) => Math.round((v / scale) * 100) + '%';
    return { phys, cog, sens, total, physPct: pct(phys), cogPct: pct(cog), sensPct: pct(sens) };
  });
</script>

<div class="page-header">
  <div>
    <div class="page-title">Activity</div>
    <div class="page-subtitle">Log time spent — energy cost is set automatically from the activity</div>
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
  <div class="col">
    <div class="list-card">
      <div class="list-header">
        <span class="card-heading">Today's activities</span>
        <span class="list-count">{entries.length} logged</span>
      </div>
      {#if loading}
        <p class="empty-list">Loading…</p>
      {:else}
        {#each rowTypeIds as typeId}
          {@const t = getType(typeId)}
          <div class="act-row">
            <div class="act-info">
              <div class="act-name">{t?.name ?? 'Unknown'}</div>
              <div class="act-cat">{getCategoryName(t?.category_id)}</div>
            </div>
            <div class="dur-field">
              <input
                type="number" step="0.25" min="0" placeholder="0"
                bind:value={durations[typeId]}
                onchange={() => saveRow(typeId)}
              />
              <span class="dur-unit">h</span>
            </div>
            {#if durations[typeId]}
              <button class="row-clear" onclick={() => clearRow(typeId)} aria-label="Clear">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
              </button>
            {:else}
              <span class="row-clear-spacer"></span>
            {/if}
          </div>
        {/each}
      {/if}

      <div class="add-row">
        <div class="select-wrap">
          <select bind:value={addTypeId} aria-label="Add activity">
            <option value={null}>Add activity…</option>
            {#each addableTypes as t}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
          <span class="select-chevron">▾</span>
        </div>
        <div class="dur-field">
          <input type="number" step="0.25" min="0" placeholder="0" bind:value={addDuration} />
          <span class="dur-unit">h</span>
        </div>
        <button class="add-btn" onclick={addActivity} disabled={!addTypeId}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
          Add
        </button>
      </div>
    </div>
  </div>

  <div class="col">
    <div class="card">
      <div>
        <div class="card-heading">Today's load</div>
        <div class="card-subtitle">From the activities logged</div>
      </div>
      <div class="load-bars">
        <div class="load-item">
          <div class="load-header">
            <span class="load-name"><span class="load-swatch" style="background:var(--accent);"></span>Physical</span>
            <span class="load-val">{loadBuckets.phys.toFixed(1)}</span>
          </div>
          <div class="bar-track"><div class="bar-fill" style="width:{loadBuckets.physPct};background:var(--accent);"></div></div>
        </div>
        <div class="load-item">
          <div class="load-header">
            <span class="load-name"><span class="load-swatch" style="background:var(--peri);"></span>Cognitive</span>
            <span class="load-val">{loadBuckets.cog.toFixed(1)}</span>
          </div>
          <div class="bar-track"><div class="bar-fill" style="width:{loadBuckets.cogPct};background:var(--peri);"></div></div>
        </div>
        <div class="load-item">
          <div class="load-header">
            <span class="load-name"><span class="load-swatch" style="background:var(--amber);"></span>Sensory / social</span>
            <span class="load-val">{loadBuckets.sens.toFixed(1)}</span>
          </div>
          <div class="bar-track"><div class="bar-fill" style="width:{loadBuckets.sensPct};background:var(--amber);"></div></div>
        </div>
      </div>
      <div class="total-box">
        <div>
          <div class="total-label">Total load</div>
          <div class="total-val">{loadBuckets.total.toFixed(1)}</div>
        </div>
        <span class="total-tag">{loadBuckets.cog > loadBuckets.phys ? 'Cognitive-heavy' : loadBuckets.phys > 0 ? 'Physically active' : 'Light day'}</span>
      </div>
      <div class="load-note">Default activities &amp; energy weights are set in Settings. Activities feed today's PEM risk.</div>
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

  .two-col { display:grid; grid-template-columns:1.5fr 1fr; gap:16px; align-items:start; }
  .col { display:flex; flex-direction:column; gap:16px; }

  .card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:16px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; }

  .list-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:8px 0; box-shadow:var(--shadow); }
  .list-header { display:flex; justify-content:space-between; align-items:center; padding:14px 20px 12px; }
  .list-count { font-size:12px; color:var(--tm); font-weight:600; }
  .empty-list { color:var(--ts); text-align:center; padding:24px; font-size:13px; }

  .act-row { display:flex; align-items:center; gap:14px; padding:11px 20px; border-top:1px solid var(--border); }
  .act-info { flex:1; min-width:0; }
  .act-name { font-size:13.5px; font-weight:600; color:var(--tp); }
  .act-cat { font-size:11.5px; color:var(--tm); }
  .dur-field { display:flex; align-items:center; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px 6px; width:92px; }
  .dur-field input { width:100%; background:transparent; border:none; padding:6px; font-size:13.5px; color:var(--tp); font-variant-numeric:tabular-nums; text-align:right; }
  .dur-unit { font-size:12px; color:var(--tm); padding-right:4px; }
  .row-clear { width:26px;height:26px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer;flex-shrink:0; }
  .row-clear-spacer { width:26px; flex-shrink:0; }

  .add-row { display:flex; align-items:center; gap:10px; padding:14px 20px; border-top:1px solid var(--border); }
  .select-wrap { position:relative; flex:1; }
  .select-wrap select { width:100%; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:11px 34px 11px 13px; font-size:13.5px; color:var(--tp); cursor:pointer; appearance:none; }
  .select-chevron { position:absolute; right:13px; top:50%; transform:translateY(-50%); color:var(--tm); pointer-events:none; font-size:11px; }
  .add-btn { display:inline-flex; align-items:center; gap:6px; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:10px 16px; font-size:13px; font-weight:700; cursor:pointer; white-space:nowrap; }
  .add-btn:disabled { opacity:.5; cursor:not-allowed; }

  .load-bars { display:flex; flex-direction:column; gap:14px; }
  .load-item { display:flex; flex-direction:column; gap:7px; }
  .load-header { display:flex; justify-content:space-between; font-size:12.5px; }
  .load-name { color:var(--tp); font-weight:600; display:inline-flex; align-items:center; gap:7px; }
  .load-swatch { width:9px; height:9px; border-radius:3px; flex-shrink:0; }
  .load-val { color:var(--ts); font-variant-numeric:tabular-nums; font-weight:700; }
  .bar-track { height:9px; border-radius:999px; background:var(--inset); overflow:hidden; }
  .bar-fill { height:100%; border-radius:999px; }

  .total-box { background:var(--inset); border-radius:14px; padding:14px 16px; display:flex; justify-content:space-between; align-items:center; }
  .total-label { font-size:10.5px; letter-spacing:.06em; text-transform:uppercase; font-weight:800; color:var(--ts); }
  .total-val { font-family:'Source Serif 4',serif; font-size:26px; font-weight:600; color:var(--tp); }
  .total-tag { font-size:11.5px; font-weight:700; color:var(--amber-fg); background:var(--amber-soft); padding:4px 11px; border-radius:999px; }
  .load-note { font-size:11.5px; color:var(--ts); line-height:1.5; }
</style>
