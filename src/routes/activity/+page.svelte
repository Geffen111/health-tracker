<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDateLong } from '$lib/formatDate';

  let today = $state(new Date().toISOString().split('T')[0]);
  let selectedDate = $state(today);
  let categories = $state<any[]>([]);
  let activityTypes = $state<any[]>([]);
  let entries = $state<any[]>([]);
  let loading = $state(true);
  let darkMode = $state(false);

  let formCategoryId = $state<number | null>(null);
  let formActivityTypeId = $state<number | null>(null);
  let formDurationHours = $state(0.5);
  let formEnergyCost = $state('Medium');

  let totalHours = $derived(entries.reduce((s: number, e: any) => s + e.duration_hours, 0));

  let totalEnergyImpact = $derived.by(() => {
    let t = 0;
    for (const entry of entries) {
      const type = activityTypes.find((at: any) => at.id === entry.activity_type_id);
      if (!type) continue;
      const cat = categories.find((c: any) => c.id === type.category_id);
      if (cat) t += entry.duration_hours * (cat.energy_weight ?? 1);
    }
    return Math.round(t * 100) / 100;
  });

  onMount(async () => {
    try {
      categories = await invoke('list_activity_categories');
      await loadTypes(null);
      await loadEntries();
    } catch (e) {
      console.error('Error loading activity data:', e);
    } finally {
      loading = false;
    }
  });

  async function loadTypes(categoryId: number | null) {
    activityTypes = await invoke('list_activity_types', { categoryId });
  }

  async function loadEntries() {
    entries = await invoke('get_activities_for_date', { date: selectedDate });
  }

  function onDateChange() {
    loadTypes(null);
    formCategoryId = null;
    formActivityTypeId = null;
    loadEntries();
  }

  function onCategoryChange() {
    formActivityTypeId = null;
    formEnergyCost = 'Medium';
    loadTypes(formCategoryId);
  }

  function onTypeChange() {
    const t = activityTypes.find((at: any) => at.id === formActivityTypeId);
    if (t?.default_energy_cost) formEnergyCost = t.default_energy_cost;
  }

  async function addActivity() {
    if (!formActivityTypeId || !formDurationHours) return;
    await invoke('add_activity_entry', {
      entry: {
        log_date: selectedDate,
        activity_type_id: formActivityTypeId,
        duration_hours: formDurationHours,
        energy_cost: formEnergyCost,
        notes: null,
      },
    });
    formCategoryId = null;
    formActivityTypeId = null;
    formDurationHours = 0.5;
    formEnergyCost = 'Medium';
    await loadTypes(null);
    await loadEntries();
  }

  async function deleteEntry(id: number) {
    await invoke('delete_activity_entry', { id });
    await loadEntries();
  }

  function getCategoryName(catId: number): string {
    return categories.find((c: any) => c.id === catId)?.name ?? '';
  }

  function getTypeName(typeId: number): string {
    return activityTypes.find((t: any) => t.id === typeId)?.name ?? 'Unknown';
  }

  function getTypeCategoryId(typeId: number): number | null {
    return activityTypes.find((t: any) => t.id === typeId)?.category_id ?? null;
  }

  let loadBuckets = $derived.by(() => {
    let phys = 0, cog = 0, sens = 0;
    for (const entry of entries) {
      const type = activityTypes.find((at: any) => at.id === entry.activity_type_id);
      if (!type) continue;
      const cat = categories.find((c: any) => c.id === type.category_id);
      if (!cat) continue;
      const weight = entry.energy_cost === 'Low' ? 0.7 : entry.energy_cost === 'High' ? 2.0 : 1.0;
      const v = entry.duration_hours * (cat.energy_weight ?? 1) * weight;
      const name = (cat.name ?? '').toLowerCase();
      if (name.includes('physical') || name.includes('domestic') || name === 'active') phys += v;
      else if (name.includes('cognitive') || name.includes('hobby')) cog += v;
      else sens += v;
    }
    const total = phys + cog + sens;
    const scale = Math.max(total > 0 ? Math.max(phys, cog, sens) : 1, 0.001);
    const pct = (v: number) => Math.round((v / scale) * 100) + '%';
    return { phys, cog, sens, total, physPct: pct(phys), cogPct: pct(cog), sensPct: pct(sens) };
  });

  function prevDay() {
    const d = new Date(selectedDate + 'T00:00:00');
    d.setDate(d.getDate() - 1);
    selectedDate = d.toISOString().split('T')[0];
    onDateChange();
  }

  function nextDay() {
    const d = new Date(selectedDate + 'T00:00:00');
    d.setDate(d.getDate() + 1);
    selectedDate = d.toISOString().split('T')[0];
    onDateChange();
  }

  function toggleTheme() {
    darkMode = !darkMode;
    document.documentElement.classList.toggle('dark', darkMode);
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Activity</div>
    <div class="page-subtitle">Log what you did — it feeds today's load in the PEM model</div>
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
    <button class="theme-btn" onclick={toggleTheme} aria-label="Toggle theme">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/></svg>
    </button>
  </div>
</div>

<div class="two-col">
  <div class="col">
    <div class="card">
      <div class="card-heading">Add an activity</div>
      <div class="add-grid">
        <div class="select-field">
          <label for="cat">Category</label>
          <div class="select-wrap">
            <select id="cat" bind:value={formCategoryId} onchange={onCategoryChange}>
              <option value={null}>All categories</option>
              {#each categories as cat}
                <option value={cat.id}>{cat.name}</option>
              {/each}
            </select>
            <span class="select-chevron">▾</span>
          </div>
        </div>
        <div class="select-field">
          <label for="type">Type</label>
          <div class="select-wrap">
            <select id="type" bind:value={formActivityTypeId} onchange={onTypeChange}>
              <option value={null}>Select type</option>
              {#each activityTypes as t}
                <option value={t.id}>{t.name}</option>
              {/each}
            </select>
            <span class="select-chevron">▾</span>
          </div>
        </div>
      </div>
      <div class="add-grid">
        <div class="text-field">
          <label for="dur">Duration</label>
          <div class="input-unit">
            <input id="dur" type="number" step="0.25" min="0" bind:value={formDurationHours} />
            <span class="unit-label">hrs</span>
          </div>
        </div>
        <div class="seg-field" role="radiogroup" aria-label="Energy cost">
          <div class="seg-control">
            <button class="seg-btn" class:active={formEnergyCost === 'Low'} onclick={() => formEnergyCost = 'Low'}>Low</button>
            <button class="seg-btn" class:active={formEnergyCost === 'Medium'} onclick={() => formEnergyCost = 'Medium'}>Medium</button>
            <button class="seg-btn" class:active={formEnergyCost === 'High'} onclick={() => formEnergyCost = 'High'}>High</button>
          </div>
        </div>
      </div>
      <button class="add-btn" onclick={addActivity}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
        Add activity
      </button>
    </div>

    <div class="list-card">
      <div class="list-header">
        <span class="card-heading">Today's activities</span>
        <span class="list-count">{entries.length} logged</span>
      </div>
      {#if entries.length === 0}
        <p class="empty-list">No activities logged for this date.</p>
      {:else}
        {#each entries as entry}
          <div class="list-row">
            <span class="list-dot" style="background:var(--accent);"></span>
            <div class="list-info">
              <div class="list-type">{getTypeName(entry.activity_type_id)}</div>
              <div class="list-cat">{getCategoryName(getTypeCategoryId(entry.activity_type_id) ?? 0)}</div>
            </div>
            <span class="list-dur">{entry.duration_hours}h</span>
            <span class="energy-badge" class:low={entry.energy_cost === 'Low'} class:med={entry.energy_cost === 'Medium'} class:high={entry.energy_cost === 'High'}>{entry.energy_cost ?? 'Medium'}</span>
            <button class="delete-btn" onclick={() => deleteEntry(entry.id)} aria-label="Delete">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
            </button>
          </div>
        {/each}
      {/if}
    </div>
  </div>

  <div class="col">
    <div class="card">
      <div>
        <div class="card-heading">Today's load</div>
        <div class="card-subtitle">From the activities logged above</div>
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
      <div class="load-note">Activities contribute to today's PEM risk calculation on the PEM Model screen.</div>
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
  .theme-btn { width:36px; height:36px; border-radius:50%; border:1px solid var(--border); background:var(--card); color:var(--ts); display:flex; align-items:center; justify-content:center; cursor:pointer; }

  .two-col { display:grid; grid-template-columns:1.5fr 1fr; gap:16px; align-items:start; }
  .col { display:flex; flex-direction:column; gap:16px; }

  .card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:16px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; }

  .add-grid { display:grid; grid-template-columns:1fr 1fr; gap:14px; }
  .select-field { display:flex; flex-direction:column; gap:7px; }
  .select-field label { font-size:12px; font-weight:700; color:var(--ts); }
  .select-wrap { position:relative; }
  .select-wrap select { width:100%; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:11px 34px 11px 13px; font-size:13.5px; color:var(--tp); cursor:pointer; appearance:none; }
  .select-chevron { position:absolute; right:13px; top:50%; transform:translateY(-50%); color:var(--tm); pointer-events:none; font-size:11px; }

  .text-field { display:flex; flex-direction:column; gap:7px; }
  .text-field label { font-size:12px; font-weight:700; color:var(--ts); }
  .input-unit { display:flex; align-items:center; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:4px 6px; }
  .input-unit input { width:100%; background:transparent; border:none; padding:7px; font-size:13.5px; color:var(--tp); font-variant-numeric:tabular-nums; }
  .unit-label { font-size:12px; color:var(--tm); padding-right:8px; }

  .seg-field { display:flex; flex-direction:column; gap:7px; }
  .seg-control { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:12px; padding:3px; gap:2px; }
  .seg-btn { flex:1; background:transparent; border:none; border-radius:9px; padding:8px 6px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .seg-btn.active { background:var(--accent); color:#fff; }

  .add-btn { align-self:flex-start; display:inline-flex; align-items:center; gap:7px; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:10px 18px; font-size:13px; font-weight:700; cursor:pointer; }

  .list-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:8px 0; box-shadow:var(--shadow); }
  .list-header { display:flex; justify-content:space-between; align-items:center; padding:14px 20px 12px; }
  .list-count { font-size:12px; color:var(--tm); font-weight:600; }
  .empty-list { color:var(--ts); text-align:center; padding:24px; font-size:13px; }

  .list-row { display:flex; align-items:center; gap:14px; padding:13px 20px; border-top:1px solid var(--border); }
  .list-dot { width:10px; height:10px; border-radius:3px; flex-shrink:0; }
  .list-info { flex:1; min-width:0; }
  .list-type { font-size:13.5px; font-weight:600; color:var(--tp); }
  .list-cat { font-size:11.5px; color:var(--tm); }
  .list-dur { font-size:13px; color:var(--ts); font-variant-numeric:tabular-nums; white-space:nowrap; }
  .energy-badge { font-size:11px; font-weight:700; padding:3px 10px; border-radius:999px; white-space:nowrap; }
  .energy-badge.low { color:var(--accent-fg); background:var(--accent-soft); }
  .energy-badge.med { color:var(--ts); background:var(--inset); border:1px solid var(--border); }
  .energy-badge.high { color:var(--amber-fg); background:var(--amber-soft); }
  .delete-btn { width:28px;height:28px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer;flex-shrink:0; }

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