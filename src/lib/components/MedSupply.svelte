<script lang="ts">
  // Supply & scripts panel for one medication, opened from its row on the Medication page:
  // record a script fill / top-up, and set the counter up (tablets per box, a day's use,
  // what's on hand now, repeats left, the new-script flag). See $lib/medSupply.ts.
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, untrack } from 'svelte';
  import { todayISO, formatDate } from '$lib/formatDate';
  import { showToast } from '$lib/stores/toast.svelte';
  import { onHandAt, daysLeft, usualFill } from '$lib/medSupply';

  let { med, dosesPerDay = 0, onchange, onclose } = $props<{
    med: any;
    /** Quick-add slots in the med's schedule — a hint for "used a day". */
    dosesPerDay?: number;
    onchange: () => void | Promise<void>;
    onclose: () => void;
  }>();

  interface Fill { id: number; fill_date: string; quantity: number; note: string | null; }
  let fills = $state<Fill[]>([]);

  const num = (v: unknown) => (v == null ? '' : String(v));
  const parse = (s: string): number | null => {
    const n = parseFloat(String(s).trim());
    return Number.isFinite(n) ? n : null;
  };
  const fmt = (n: number | null) => (n == null ? '—' : String(Math.round(n * 10) / 10));

  // Settings start in the form when the counter isn't set up yet.
  let tab = $state<'fill' | 'settings'>(untrack(() => med.supply_tracking && med.supply_per_day ? 'fill' : 'settings'));

  // ── Fill ──
  function repeatsAfter(): string {
    return med.script_repeats_left == null ? '' : String(Math.max(0, med.script_repeats_left - 1));
  }
  // The forms start from the med as it was when the panel opened.
  let fill = $state(untrack(() => ({
    date: todayISO(),
    boxes: num(med.supply_boxes_per_fill ?? 1),
    boxSize: num(med.supply_box_size),
    repeats: repeatsAfter(),
    needsScript: med.script_repeats_left != null ? med.script_repeats_left - 1 <= 0 : !!med.needs_script,
    note: '',
  })));
  let fillQty = $derived((parse(fill.boxes) ?? 0) * (parse(fill.boxSize) ?? 0));
  // Ticking the flag follows the repeats while they're being edited.
  function onRepeatsInput() {
    const r = parse(fill.repeats);
    if (r != null) fill.needsScript = r <= 0;
  }

  async function saveFill() {
    if (!(fillQty > 0)) return;
    try {
      await invoke('record_medication_fill', {
        id: med.id,
        fillDate: fill.date,
        quantity: fillQty,
        repeatsLeft: fill.repeats.trim() === '' ? null : Math.round(parse(fill.repeats) ?? 0),
        needsScript: fill.needsScript,
        boxSize: parse(fill.boxSize),
        note: fill.note || null,
      });
      showToast(`${med.name}: ${fmt(fillQty)} added`);
      await onchange();
      onclose();
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  // ── Settings ──
  let set = $state(untrack(() => ({
    tracking: med.supply_tracking || !med.supply_per_day,
    onHand: fmt(onHandAt(med)) === '—' ? '' : fmt(onHandAt(med)),
    perDay: num(med.supply_per_day ?? (dosesPerDay || null)),
    boxSize: num(med.supply_box_size),
    boxes: num(med.supply_boxes_per_fill ?? 1),
    warnDays: num(med.supply_warn_days ?? 7),
    repeats: num(med.script_repeats_left),
    needsScript: !!med.needs_script,
  })));
  const onHandStart = set.onHand;

  function settingsPayload(onHand: number | null) {
    return {
      tracking: set.tracking,
      on_hand: onHand,
      per_day: parse(set.perDay),
      box_size: parse(set.boxSize),
      boxes_per_fill: parse(set.boxes) ?? 1,
      warn_days: Math.round(parse(set.warnDays) ?? 7),
      repeats_left: set.repeats.trim() === '' ? null : Math.round(parse(set.repeats) ?? 0),
      needs_script: set.needsScript,
    };
  }

  async function saveSettings() {
    try {
      // Only a changed "on hand" resets the count (and its date) — otherwise it keeps running.
      const onHand = set.onHand !== onHandStart ? parse(set.onHand) ?? 0 : null;
      await invoke('save_medication_supply', { id: med.id, supply: settingsPayload(onHand) });
      showToast('Supply settings saved');
      await onchange();
      onclose();
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  async function undoFill(f: Fill) {
    try {
      await invoke('delete_medication_fill', { id: f.id });
      fills = fills.filter((x) => x.id !== f.id);
      showToast(`Fill removed — ${fmt(f.quantity)} taken off the count`);
      await onchange();
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  onMount(async () => {
    try { fills = await invoke<Fill[]>('list_medication_fills', { medicationId: med.id }); } catch {}
  });

  let left = $derived(daysLeft(med));
  const unit = 'tablets';
</script>

<div class="supply">
  <div class="supply-head">
    <span class="supply-title">Supply &amp; scripts</span>
    {#if med.supply_tracking && left != null}
      <span class="supply-now">{fmt(onHandAt(med))} {unit} on hand · {left} day{left === 1 ? '' : 's'} left</span>
    {/if}
    <div class="seg">
      <button class:active={tab === 'fill'} onclick={() => tab = 'fill'}>Script filled</button>
      <button class:active={tab === 'settings'} onclick={() => tab = 'settings'}>Settings</button>
    </div>
  </div>

  {#if tab === 'fill'}
    <div class="row">
      <label>Date <input type="date" bind:value={fill.date} max={todayISO()} /></label>
      <label>Boxes <input class="n" bind:value={fill.boxes} inputmode="decimal" /></label>
      <span class="x">×</span>
      <label>{unit} per box <input class="n" bind:value={fill.boxSize} inputmode="decimal" /></label>
      <span class="eq">= <strong>{fmt(fillQty)}</strong> {unit}</span>
    </div>
    <div class="row">
      <label>Repeats left after this <input class="n" bind:value={fill.repeats} oninput={onRepeatsInput} placeholder="—" inputmode="numeric" /></label>
      <label class="check"><input type="checkbox" bind:checked={fill.needsScript} /> Need a new script</label>
      <input class="note" bind:value={fill.note} placeholder="Note (optional)" />
    </div>
    <div class="row end">
      <button class="cancel" onclick={onclose}>Cancel</button>
      <button class="save" onclick={saveFill} disabled={!(fillQty > 0)}>Add {fmt(fillQty)} {unit}</button>
    </div>
    {#if !usualFill(med)}
      <p class="hint">The box size is remembered for next time.</p>
    {/if}
  {:else}
    <div class="row">
      <label class="check"><input type="checkbox" bind:checked={set.tracking} /> Show days left</label>
    </div>
    <div class="row">
      <label>On hand now <input class="n" bind:value={set.onHand} placeholder="0" inputmode="decimal" /></label>
      <label>Used a day <input class="n" bind:value={set.perDay} inputmode="decimal" /></label>
      {#if dosesPerDay && parse(set.perDay) !== dosesPerDay}
        <button class="link" onclick={() => set.perDay = String(dosesPerDay)}>Schedule: {dosesPerDay}/day</button>
      {/if}
      <label>{unit} per box <input class="n" bind:value={set.boxSize} inputmode="decimal" /></label>
      <label>Boxes per fill <input class="n" bind:value={set.boxes} inputmode="decimal" /></label>
    </div>
    <div class="row">
      <label>Repeats left <input class="n" bind:value={set.repeats} placeholder="—" inputmode="numeric" /></label>
      <label class="check"><input type="checkbox" bind:checked={set.needsScript} /> Need a new script</label>
      <label>Warn at <input class="n" bind:value={set.warnDays} inputmode="numeric" /> days left</label>
    </div>
    <div class="row end">
      <button class="cancel" onclick={onclose}>Cancel</button>
      <button class="save" onclick={saveSettings}>Save</button>
    </div>
  {/if}

  {#if fills.length}
    <div class="fills">
      <span class="fills-label">Fills</span>
      {#each fills.slice(0, 5) as f, i (f.id)}
        <span class="fill">
          {formatDate(f.fill_date)} · +{fmt(f.quantity)}{#if f.note} · {f.note}{/if}
          {#if i === 0}<button class="link" onclick={() => undoFill(f)} title="Remove this fill and take its tablets off the count">Undo</button>{/if}
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .supply { display:flex; flex-direction:column; gap:10px; padding:12px 18px; background:var(--inset); border-top:1px solid var(--border); }
  .supply-head { display:flex; align-items:center; gap:12px; flex-wrap:wrap; }
  .supply-title { font-size:12.5px; font-weight:700; color:var(--tp); }
  .supply-now { font-size:12px; color:var(--ts); font-variant-numeric:tabular-nums; }
  .seg { margin-left:auto; display:flex; background:var(--card); border:1px solid var(--border); border-radius:999px; padding:2px; }
  .seg button { background:none; border:none; border-radius:999px; padding:5px 11px; font-size:11.5px; font-weight:600; color:var(--ts); cursor:pointer; }
  .seg button.active { background:var(--accent-soft); color:var(--accent-fg); }
  .row { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .row.end { justify-content:flex-end; }
  label { display:flex; align-items:center; gap:6px; font-size:11.5px; color:var(--ts); font-weight:600; }
  input { background:var(--card); border:1px solid var(--border); border-radius:9px; padding:6px 8px; font-size:12.5px; color:var(--tp); }
  input.n { width:58px; text-align:center; font-variant-numeric:tabular-nums; }
  input.note { flex:1; min-width:140px; }
  label.check input { width:auto; }
  .x, .eq { font-size:12px; color:var(--ts); }
  .eq strong { color:var(--tp); }
  .save { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:8px 15px; font-size:12px; font-weight:700; cursor:pointer; }
  .save:disabled { opacity:.5; cursor:default; }
  .cancel { background:transparent; border:none; color:var(--ts); font-size:12px; font-weight:600; cursor:pointer; }
  .link { background:none; border:none; padding:0; color:var(--accent-fg); font-size:11.5px; font-weight:600; cursor:pointer; text-decoration:underline; }
  .hint { margin:0; font-size:11.5px; color:var(--tm); }
  .fills { display:flex; flex-wrap:wrap; gap:6px 14px; align-items:center; font-size:11.5px; color:var(--ts); border-top:1px solid var(--border); padding-top:8px; }
  .fills-label { font-weight:700; color:var(--tm); text-transform:uppercase; font-size:10.5px; letter-spacing:.04em; }
  .fill { display:inline-flex; gap:6px; align-items:center; }
</style>
