<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { showToast } from '$lib/stores/toast.svelte';
  import type { Food, CleanupSuggestion } from '$lib/food';

  // "Tidy up": tags anything untagged, then asks the model for clean-ups. Nothing in the
  // log changes until a suggestion is applied — merges and splits rewrite past entries.

  let { foods, onchange, onclose }: {
    foods: Food[];
    onchange: () => Promise<void> | void;
    onclose: () => void;
  } = $props();

  interface Row extends CleanupSuggestion { edit: string; }

  let stage = $state<'tagging' | 'thinking' | 'done'>('tagging');
  let tagged = $state(0);
  let error = $state('');
  let rows = $state<Row[]>([]);
  let busy = $state(false);

  const byId = (id: number) => foods.find((f) => f.id === id);
  // Hide a suggestion once an earlier one has merged or split one of its items away.
  let live = $derived(rows.filter((r) => r.food_ids.every((id) => byId(id))));

  onMount(async () => {
    try {
      tagged = await invoke<number>('tag_untagged_foods');
      if (tagged) await onchange();
      stage = 'thinking';
      const found = await invoke<CleanupSuggestion[]>('suggest_food_cleanup');
      rows = found.map((s) => ({ ...s, edit: s.names.join(', ') }));
    } catch (e) {
      error = String(e);
    } finally {
      stage = 'done';
    }
  });

  function names(r: Row) { return r.food_ids.map((id) => byId(id)?.name ?? '?'); }
  function dismiss(r: Row) { rows = rows.filter((x) => x !== r); }

  async function apply(r: Row) {
    const parts = r.action === 'split'
      ? r.edit.split(',').map((s) => s.trim()).filter(Boolean)
      : [r.edit.trim()];
    if (!parts[0] || (r.action === 'split' && parts.length < 2)) return;
    busy = true;
    try {
      if (r.action === 'merge') {
        // Keep the most-logged one, so its tags (and any hand edits) survive.
        const items = r.food_ids.map(byId).filter((f): f is Food => !!f)
          .sort((a, b) => b.days_logged - a.days_logged);
        await invoke('merge_foods', { keepId: items[0].id, otherIds: items.slice(1).map((f) => f.id), name: parts[0] });
      } else if (r.action === 'rename') {
        const f = byId(r.food_ids[0])!;
        await invoke('update_food', { id: f.id, name: parts[0], kind: f.kind, regular: f.regular, active: f.active });
      } else {
        await invoke('split_food', { id: r.food_ids[0], names: parts });
        invoke('tag_untagged_foods').then(() => onchange()).catch(() => {});
      }
      dismiss(r);
      await onchange();
    } catch (e) {
      showToast(String(e), 'error');
    } finally {
      busy = false;
    }
  }

  const verb = { merge: 'Merge', rename: 'Rename', split: 'Split' };
</script>

<div class="form-card">
  <div class="head">
    <div>
      <div class="card-heading">Tidy up the food list</div>
      <div class="card-subtitle">
        {#if stage === 'tagging'}<span class="spinner"></span>Tagging new items…
        {:else if stage === 'thinking'}{tagged ? `Tagged ${tagged} item${tagged === 1 ? '' : 's'}. ` : ''}<span class="spinner"></span>Looking for duplicates and odd names…
        {:else if error}<span class="err">{error}</span>
        {:else}{tagged ? `Tagged ${tagged} item${tagged === 1 ? '' : 's'}. ` : 'Every item is tagged. '}{live.length ? 'Nothing below changes until you apply it.' : 'No clean-ups to suggest.'}{/if}
      </div>
    </div>
    <button class="ghost-btn" onclick={onclose}>Done</button>
  </div>

  {#each live as r (r)}
    <div class="sugg">
      <div class="sugg-text">
        <span class="verb">{verb[r.action]}</span>
        {names(r).join(r.action === 'merge' ? ' + ' : '')}
        <span class="arrow">→</span>
        <input class="target" bind:value={r.edit} aria-label="Result"
          title={r.action === 'split' ? 'Parts, separated by commas' : 'Name to use'} />
        {#if r.reason}<div class="reason">{r.reason}</div>{/if}
      </div>
      <button class="save-sm" onclick={() => apply(r)} disabled={busy}>Apply</button>
      <button class="cancel-sm" onclick={() => dismiss(r)}>Dismiss</button>
    </div>
  {/each}
  {#if live.length && rows.some((r) => r.action !== 'rename')}
    <div class="note">Merging moves every logged entry onto one item; splitting turns each entry into one per part. Past days keep their entries, just relabelled.</div>
  {/if}
</div>

<style>
  .form-card { background:var(--card);border:1px solid var(--border);border-radius:18px;padding:20px;box-shadow:var(--shadow);margin-bottom:16px;display:flex;flex-direction:column;gap:10px; }
  .head { display:flex; justify-content:space-between; align-items:flex-start; gap:12px; margin-bottom:4px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:3px; display:flex; align-items:center; gap:7px; flex-wrap:wrap; }
  .err { color:var(--red-fg); }
  .ghost-btn { background:var(--card);color:var(--ts);border:1px solid var(--border);border-radius:999px;padding:8px 16px;font-size:13px;font-weight:700;cursor:pointer; }
  .spinner { display:inline-block; width:12px; height:12px; border-radius:50%; border:2px solid var(--border); border-top-color:var(--accent); animation:spin .8s linear infinite; }
  @keyframes spin { to { transform:rotate(360deg); } }
  .sugg { display:flex; align-items:center; gap:10px; padding:10px 12px; background:var(--inset); border-radius:12px; }
  .sugg-text { flex:1; min-width:0; font-size:13px; color:var(--tp); display:flex; align-items:center; gap:7px; flex-wrap:wrap; }
  .verb { font-size:10.5px; font-weight:800; letter-spacing:.05em; text-transform:uppercase; color:var(--accent-fg); background:var(--accent-soft); border-radius:999px; padding:2px 8px; }
  .arrow { color:var(--tm); }
  .target { min-width:140px; flex:1; max-width:320px; background:var(--card); border:1px solid var(--border); border-radius:8px; padding:5px 8px; font-size:13px; color:var(--tp); }
  .reason { width:100%; font-size:11.5px; color:var(--tm); }
  .save-sm { background:var(--accent);color:#fff;border:none;border-radius:999px;padding:7px 14px;font-size:12px;font-weight:700;cursor:pointer; }
  .save-sm:disabled { opacity:.5; cursor:not-allowed; }
  .cancel-sm { background:transparent;border:none;color:var(--ts);font-size:12px;font-weight:600;cursor:pointer; }
  .note { font-size:11.5px; color:var(--tm); line-height:1.5; }
</style>
