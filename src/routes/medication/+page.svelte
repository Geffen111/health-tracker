<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let medications = $state<any[]>([]);
  let loading = $state(true);

  let showAddForm = $state(false);
  let newName = $state('');
  let newCode = $state('');
  let newDose = $state<number | null>(null);
  let newUnit = $state('mg');
  let newCategory = $state('');

  onMount(async () => {
    try {
      medications = await invoke('list_medications');
    } catch (e) {
      console.error('Error loading medications:', e);
    } finally {
      loading = false;
    }
  });

  async function addMedication() {
    if (!newName.trim()) return;
    const med = await invoke('create_medication', {
      name: newName.trim(),
      shortCode: newCode.trim() || null,
      defaultDose: newDose,
      doseUnit: newUnit || 'mg',
      category: newCategory.trim() || null,
    });
    medications = [...medications, med];
    newName = '';
    newCode = '';
    newDose = null;
    newUnit = 'mg';
    newCategory = '';
    showAddForm = false;
  }

  async function archiveMed(id: number) {
    await invoke('archive_medication', { id });
    medications = medications.filter(m => m.id !== id);
  }
</script>

<h1>Medication</h1>

<div class="toolbar">
  <button class="add-btn" onclick={() => showAddForm = !showAddForm}>
    {showAddForm ? 'Cancel' : '+ Add Medication'}
  </button>
</div>

{#if showAddForm}
  <div class="add-form">
    <input bind:value={newName} placeholder="Medication name" />
    <input bind:value={newCode} placeholder="Short code (e.g. Dex)" />
    <input type="number" bind:value={newDose} placeholder="Default dose" />
    <select bind:value={newUnit}>
      <option value="mg">mg</option>
      <option value="mcg">mcg</option>
      <option value="mL">mL</option>
      <option value="tablets">tablets</option>
    </select>
    <input bind:value={newCategory} placeholder="Category (e.g. stimulant)" />
    <button onclick={addMedication}>Save</button>
  </div>
{/if}

{#if loading}
  <p>Loading...</p>
{:else if medications.length === 0}
  <p class="empty">No medications added yet. Add your first one above.</p>
{:else}
  <div class="med-list">
    {#each medications as med}
      <div class="med-card">
        <div class="med-info">
          <span class="med-name">{med.name}</span>
          {#if med.short_code}
            <span class="med-code">{med.short_code}</span>
          {/if}
          {#if med.default_dose}
            <span class="med-dose">{med.default_dose}{med.dose_unit || 'mg'}</span>
          {/if}
          {#if med.category}
            <span class="med-category">{med.category}</span>
          {/if}
        </div>
        <button class="archive-btn" onclick={() => archiveMed(med.id)} title="Archive">🗑️</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  h1 { margin-bottom: 16px; }

  .toolbar { margin-bottom: 20px; }

  .add-btn, .add-form button {
    padding: 10px 20px;
    background: #1976d2;
    color: #fff;
    border: none;
    border-radius: 8px;
    font-weight: 600;
    cursor: pointer;
  }
  .add-btn:hover { background: #1565c0; }

  .add-form {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 16px;
    background: #fff;
    border-radius: 12px;
    margin-bottom: 20px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  }
  :global(.dark) .add-form { background: #1e2a45; }

  .add-form input, .add-form select {
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
  }
  :global(.dark) .add-form input, :global(.dark) .add-form select {
    background: #2a3a5c;
    border-color: #444;
    color: #e0e0e0;
  }

  .empty { color: #888; padding: 24px; text-align: center; }

  .med-list {
    display: grid;
    gap: 8px;
    max-width: 600px;
  }

  .med-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #fff;
    border-radius: 10px;
    padding: 14px 16px;
    box-shadow: 0 1px 2px rgba(0,0,0,0.06);
  }
  :global(.dark) .med-card { background: #1e2a45; }

  .med-info { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }

  .med-name { font-weight: 600; font-size: 15px; }
  .med-code { font-size: 12px; color: #888; background: #f0f0f0; padding: 2px 8px; border-radius: 4px; }
  :global(.dark) .med-code { background: #2a3a5c; color: #aaa; }
  .med-dose { font-size: 14px; color: #555; }
  .med-category { font-size: 12px; color: #1976d2; background: #e3f2fd; padding: 2px 8px; border-radius: 4px; }

  .archive-btn {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 16px;
    padding: 4px;
    opacity: 0.5;
    transition: opacity 0.15s;
  }
  .archive-btn:hover { opacity: 1; }
</style>