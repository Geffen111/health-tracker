<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  interface ActivityCategory {
    id: number;
    name: string;
    energy_weight: number;
  }

  interface ActivityType {
    id: number;
    name: string;
    category_id: number;
    default_energy_cost: string | null;
  }

  interface ActivityEntry {
    id: number;
    log_date: string;
    activity_type_id: number;
    duration_hours: number;
    energy_cost: string;
    notes: string | null;
  }

  let today = $state(new Date().toISOString().split('T')[0]);
  let selectedDate = $state(today);

  let categories = $state<ActivityCategory[]>([]);
  let activityTypes = $state<ActivityType[]>([]);
  let entries = $state<ActivityEntry[]>([]);
  let loading = $state(true);

  let formCategoryId = $state<number | null>(null);
  let formActivityTypeId = $state<number | null>(null);
  let formDurationHours = $state(0.5);
  let formEnergyCost = $state('Medium');
  let formNotes = $state('');

  let totalHours = $derived(entries.reduce((s, e) => s + e.duration_hours, 0));

  let totalEnergyImpact = $derived.by(() => {
    let t = 0;
    for (const entry of entries) {
      const type = activityTypes.find(at => at.id === entry.activity_type_id);
      if (!type) continue;
      const cat = categories.find(c => c.id === type.category_id);
      if (cat) t += entry.duration_hours * cat.energy_weight;
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

  async function onDateChange() {
    await loadTypes(null);
    formCategoryId = null;
    formActivityTypeId = null;
    await loadEntries();
  }

  async function onCategoryChange() {
    formActivityTypeId = null;
    formEnergyCost = 'Medium';
    await loadTypes(formCategoryId);
  }

  function onTypeChange() {
    const t = activityTypes.find(at => at.id === formActivityTypeId);
    if (t?.default_energy_cost) {
      formEnergyCost = t.default_energy_cost;
    } else {
      formEnergyCost = 'Medium';
    }
  }

  async function addActivity() {
    if (!formActivityTypeId || !formDurationHours) return;
    await invoke('add_activity_entry', {
      entry: {
        log_date: selectedDate,
        activity_type_id: formActivityTypeId,
        duration_hours: formDurationHours,
        energy_cost: formEnergyCost,
        notes: formNotes || null,
      },
    });
    resetForm();
    await loadEntries();
  }

  async function deleteEntry(id: number) {
    await invoke('delete_activity_entry', { id });
    await loadEntries();
  }

  function resetForm() {
    formCategoryId = null;
    formActivityTypeId = null;
    formDurationHours = 0.5;
    formEnergyCost = 'Medium';
    formNotes = '';
    loadTypes(null);
  }

  function getCategoryName(catId: number): string {
    return categories.find(c => c.id === catId)?.name ?? '';
  }

  function getTypeName(typeId: number): string {
    return activityTypes.find(t => t.id === typeId)?.name ?? 'Unknown';
  }

  function getTypeCategoryId(typeId: number): number | null {
    return activityTypes.find(t => t.id === typeId)?.category_id ?? null;
  }

  function energyCostClass(cost: string): string {
    if (cost === 'Low') return 'cost-low';
    if (cost === 'High') return 'cost-high';
    return 'cost-medium';
  }
</script>

<h1>Activity</h1>

<div class="toolbar">
  <label class="date-label">
    Date
    <input type="date" class="date-input" bind:value={selectedDate} onchange={onDateChange} />
  </label>
</div>

<div class="summary-card">
  <span>Total hours: <strong>{totalHours.toFixed(1)}</strong></span>
  <span>Energy impact: <strong>{totalEnergyImpact}</strong></span>
</div>

<div class="add-form">
  <h3>Add Activity</h3>
  <div class="form-row">
    <select bind:value={formCategoryId} onchange={onCategoryChange}>
      <option value={null}>All categories</option>
      {#each categories as cat}
        <option value={cat.id}>{cat.name}</option>
      {/each}
    </select>
    <select bind:value={formActivityTypeId} onchange={onTypeChange}>
      <option value={null}>Select type</option>
      {#each activityTypes as t}
        <option value={t.id}>{t.name}</option>
      {/each}
    </select>
    <input
      type="number"
      step="0.25"
      min="0"
      bind:value={formDurationHours}
      placeholder="Hours"
    />
    <select bind:value={formEnergyCost}>
      <option value="Low">Low</option>
      <option value="Medium">Medium</option>
      <option value="High">High</option>
    </select>
    <textarea
      bind:value={formNotes}
      placeholder="Notes (optional)"
      rows="1"
    ></textarea>
    <button class="add-btn" onclick={addActivity}>Add Activity</button>
  </div>
</div>

{#if loading}
  <p>Loading...</p>
{:else if entries.length === 0}
  <p class="empty">No activities logged for this date.</p>
{:else}
  <div class="entry-list">
    {#each entries as entry}
      <div class="entry-card">
        <div class="entry-info">
          <span class="entry-name">{getTypeName(entry.activity_type_id)}</span>
          <span class="category-badge">{getCategoryName(getTypeCategoryId(entry.activity_type_id) ?? 0)}</span>
          <span class="duration">{entry.duration_hours}h</span>
          <span class="energy-badge {energyCostClass(entry.energy_cost)}">{entry.energy_cost}</span>
          {#if entry.notes}
            <span class="entry-notes">{entry.notes}</span>
          {/if}
        </div>
        <button class="delete-btn" onclick={() => deleteEntry(entry.id)} title="Delete">🗑️</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  h1 { margin-bottom: 16px; }

  .toolbar { margin-bottom: 16px; }

  .date-label {
    font-size: 13px;
    font-weight: 600;
    color: #555;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  :global(.dark) .date-label { color: #bbb; }

  .date-input {
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
    font-family: inherit;
    background: #fff;
  }
  :global(.dark) .date-input {
    background: #2a3a5c;
    border-color: #444;
    color: #e0e0e0;
  }

  .summary-card {
    display: flex;
    gap: 24px;
    padding: 14px 18px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    margin-bottom: 20px;
    font-size: 14px;
  }
  :global(.dark) .summary-card { background: #1e2a45; color: #e0e0e0; }

  .add-form {
    padding: 18px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    margin-bottom: 20px;
  }
  :global(.dark) .add-form { background: #1e2a45; }

  .add-form h3 {
    margin: 0 0 12px 0;
    font-size: 15px;
  }

  .form-row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: flex-start;
  }

  .form-row select,
  .form-row input,
  .form-row textarea {
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
    font-family: inherit;
  }
  :global(.dark) .form-row select,
  :global(.dark) .form-row input,
  :global(.dark) .form-row textarea {
    background: #2a3a5c;
    border-color: #444;
    color: #e0e0e0;
  }

  .form-row textarea {
    min-width: 200px;
    resize: vertical;
  }

  .add-btn {
    padding: 8px 18px;
    background: #1976d2;
    color: #fff;
    border: none;
    border-radius: 8px;
    font-weight: 600;
    font-size: 14px;
    cursor: pointer;
    white-space: nowrap;
  }
  .add-btn:hover { background: #1565c0; }

  .empty { color: #888; padding: 24px; text-align: center; }

  .entry-list {
    display: grid;
    gap: 8px;
  }

  .entry-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #fff;
    border-radius: 10px;
    padding: 12px 16px;
    box-shadow: 0 1px 2px rgba(0,0,0,0.06);
  }
  :global(.dark) .entry-card { background: #1e2a45; }

  .entry-info {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .entry-name { font-weight: 600; font-size: 15px; }

  .category-badge {
    font-size: 11px;
    color: #1976d2;
    background: #e3f2fd;
    padding: 2px 8px;
    border-radius: 4px;
    white-space: nowrap;
  }
  :global(.dark) .category-badge {
    background: #1a3a5c;
    color: #64b5f6;
  }

  .duration {
    font-size: 14px;
    color: #555;
    white-space: nowrap;
  }
  :global(.dark) .duration { color: #aaa; }

  .energy-badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 4px;
    font-weight: 600;
    white-space: nowrap;
  }
  .cost-low { background: #e8f5e9; color: #2e7d32; }
  .cost-medium { background: #fff3e0; color: #e65100; }
  .cost-high { background: #fbe9e7; color: #c62828; }
  :global(.dark) .cost-low { background: #1b3a1b; color: #81c784; }
  :global(.dark) .cost-medium { background: #3a2a1b; color: #ffb74d; }
  :global(.dark) .cost-high { background: #3a1b1b; color: #ef9a9a; }

  .entry-notes {
    font-size: 12px;
    color: #888;
    font-style: italic;
  }
  :global(.dark) .entry-notes { color: #999; }

  .delete-btn {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 16px;
    padding: 4px;
    opacity: 0.5;
    transition: opacity 0.15s;
  }
  .delete-btn:hover { opacity: 1; }
</style>