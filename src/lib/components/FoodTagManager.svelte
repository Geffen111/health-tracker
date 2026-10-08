<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { showToast } from '$lib/stores/toast.svelte';
  import { confirmAction } from '$lib/stores/confirm.svelte';
  import type { FoodTag } from '$lib/food';

  // The two lists the AI picks from. Renaming keeps every item's tag; deleting takes it
  // off the items that had it.

  let { categories, flags, onchange, onclose }: {
    categories: FoodTag[];
    flags: FoodTag[];
    onchange: () => void;
    onclose: () => void;
  } = $props();

  let added = $state({ category: '', flag: '' });

  const cmd = (kind: 'category' | 'flag', op: 'save' | 'delete') => `${op}_food_${kind}`;

  async function save(kind: 'category' | 'flag', t: FoodTag | null, name: string) {
    name = name.trim();
    if (!name || (t && t.name === name)) return;
    try {
      await invoke(cmd(kind, 'save'), { id: t?.id ?? null, name });
      if (!t) added[kind] = '';
      onchange();
    } catch (e) {
      showToast(String(e), 'error');
      onchange(); // put the old name back in the input
    }
  }

  async function remove(kind: 'category' | 'flag', t: FoodTag) {
    if (t.items) {
      const ok = await confirmAction({
        title: `Delete ${t.name}?`,
        message: `${t.items} item${t.items === 1 ? '' : 's'} ${t.items === 1 ? 'has' : 'have'} it and will lose it.`,
        confirmLabel: 'Delete',
      });
      if (!ok) return;
    }
    try {
      await invoke(cmd(kind, 'delete'), { id: t.id });
      onchange();
    } catch (e) {
      showToast(String(e), 'error');
    }
  }
</script>

{#snippet list(kind: 'category' | 'flag', title: string, hint: string, tags: FoodTag[])}
  <div class="col">
    <div class="col-head">{title}</div>
    <div class="col-hint">{hint}</div>
    {#each tags as t (t.id)}
      <div class="tag-row">
        <input class="tag-name" value={t.name} aria-label="Name"
          onchange={(e) => save(kind, t, e.currentTarget.value)} />
        <span class="tag-count">{t.items}</span>
        <button class="del" onclick={() => remove(kind, t)} aria-label="Delete {t.name}" title="Delete">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
        </button>
      </div>
    {/each}
    <div class="tag-row add">
      <input class="tag-name" bind:value={added[kind]} placeholder="Add {kind}…"
        onkeydown={(e) => { if (e.key === 'Enter') save(kind, null, added[kind]); }} />
      <button class="add-btn" onclick={() => save(kind, null, added[kind])} disabled={!added[kind].trim()}>Add</button>
    </div>
  </div>
{/snippet}

<div class="form-card">
  <div class="head">
    <div>
      <div class="card-heading">Categories &amp; flags</div>
      <div class="card-subtitle">The AI tags new items from these lists. The number is how many items have each one.</div>
    </div>
    <button class="ghost-btn" onclick={onclose}>Done</button>
  </div>
  <div class="cols">
    {@render list('category', 'Categories', 'One per item', categories)}
    {@render list('flag', 'Flags', 'Any number per item', flags)}
  </div>
</div>

<style>
  .form-card { background:var(--card);border:1px solid var(--border);border-radius:18px;padding:20px;box-shadow:var(--shadow);margin-bottom:16px;display:flex;flex-direction:column;gap:14px; }
  .head { display:flex; justify-content:space-between; align-items:flex-start; gap:12px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; }
  .ghost-btn { background:var(--card);color:var(--ts);border:1px solid var(--border);border-radius:999px;padding:8px 16px;font-size:13px;font-weight:700;cursor:pointer; }
  .cols { display:grid; grid-template-columns:1fr 1fr; gap:20px; }
  @media (max-width: 760px) { .cols { grid-template-columns:1fr; } }
  .col { display:flex; flex-direction:column; gap:4px; }
  .col-head { font-size:10.5px; letter-spacing:.07em; text-transform:uppercase; font-weight:800; color:var(--tm); }
  .col-hint { font-size:11.5px; color:var(--tm); margin-bottom:4px; }
  .tag-row { display:flex; align-items:center; gap:8px; }
  .tag-row.add { margin-top:6px; }
  .tag-name { flex:1; min-width:0; background:transparent; border:1px solid transparent; border-radius:8px; padding:5px 8px; font-size:13px; color:var(--tp); font-family:inherit; }
  .tag-name:hover { border-color:var(--border); }
  .tag-name:focus { outline:none; border-color:var(--accent); background:var(--card); }
  .tag-row.add .tag-name { background:var(--inset); border-color:var(--border); }
  .tag-count { font-size:11.5px; color:var(--tm); font-variant-numeric:tabular-nums; min-width:22px; text-align:right; }
  .del { width:24px;height:24px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .del:hover { color:var(--red-fg); }
  .add-btn { background:var(--accent);color:#fff;border:none;border-radius:999px;padding:6px 13px;font-size:12px;font-weight:700;cursor:pointer; }
  .add-btn:disabled { opacity:.5; cursor:not-allowed; }
</style>
