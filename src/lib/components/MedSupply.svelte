<script lang="ts">
  // Supply & scripts panel for one medication, opened from its row on the Medication page.
  // A med can have several supplies — one per tablet strength or script (Candesartan 12 mg
  // taken as an 8 mg + a 4 mg tablet has two). Each has its own count, box, repeats and
  // expiry; doses logged are split over them in Rust (see $lib/medSupply.ts).
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { todayISO, formatDate } from '$lib/formatDate';
  import { showToast } from '$lib/stores/toast.svelte';
  import { confirmAction } from '$lib/stores/confirm.svelte';
  import { supplyName, supplyLevel, usualFill, fmtCount, type Supply } from '$lib/medSupply';

  let { med, supplies, onchange, onclose } = $props<{
    med: any;
    supplies: Supply[];
    onchange: () => void | Promise<void>;
    onclose: () => void;
  }>();

  interface Fill { id: number; supply_id: number | null; fill_date: string; quantity: number; note: string | null; }
  let fills = $state<Fill[]>([]);
  async function loadFills() {
    try { fills = await invoke<Fill[]>('list_medication_fills', { medicationId: med.id }); } catch {}
  }
  onMount(loadFills);

  const num = (v: unknown) => (v == null ? '' : String(v));
  const parse = (s: string): number | null => {
    const n = parseFloat(String(s).trim());
    return Number.isFinite(n) ? n : null;
  };
  const intOrNull = (s: string) => (s.trim() === '' ? null : Math.round(parse(s) ?? 0));
  let unit = $derived(med.dose_unit || 'mg');
  let inTablets = $derived(/^(tab|cap|pill)/i.test((med.dose_unit ?? '').trim()));
  const name = (s: Supply) => supplyName(s, med) || 'Tablets';

  // One form open at a time: a fill or the settings for a supply ('new' adds one).
  let open = $state<{ id: number | 'new'; kind: 'fill' | 'edit' } | null>(null);
  const isOpen = (id: number | 'new', kind: 'fill' | 'edit') => open?.id === id && open?.kind === kind;

  // ── Fill ──
  let fill = $state({ date: '', boxes: '', boxSize: '', repeats: '', needsScript: false, expiry: '', note: '' });
  let fillQty = $derived((parse(fill.boxes) ?? 0) * (parse(fill.boxSize) ?? 0));
  function openFill(s: Supply) {
    open = { id: s.id, kind: 'fill' };
    const after = s.repeats_left == null ? null : Math.max(0, s.repeats_left - 1);
    fill = {
      date: todayISO(),
      boxes: num(s.boxes_per_fill ?? 1),
      boxSize: num(s.box_size),
      repeats: num(after),
      needsScript: after != null ? after <= 0 : s.needs_script,
      expiry: s.script_expiry ?? '',
      note: '',
    };
  }
  // The flag follows the repeats while they're being edited.
  function onRepeatsInput() {
    const r = parse(fill.repeats);
    if (r != null) fill.needsScript = r <= 0;
  }
  async function saveFill(s: Supply) {
    if (!(fillQty > 0)) return;
    try {
      await invoke('record_medication_fill', {
        supplyId: s.id,
        fillDate: fill.date,
        quantity: fillQty,
        repeatsLeft: intOrNull(fill.repeats),
        needsScript: fill.needsScript,
        boxSize: parse(fill.boxSize),
        scriptExpiry: fill.expiry || null,
        note: fill.note || null,
      });
      showToast(`${med.name} ${name(s)}: ${fmtCount(fillQty)} added`);
      open = null;
      await Promise.all([onchange(), loadFills()]);
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  // ── Settings ──
  let set = $state({ strength: '', label: '', onHand: '', boxSize: '', boxes: '1', warnDays: '7', repeats: '', needsScript: false, expiry: '' });
  let onHandStart = '';
  function openEdit(s: Supply | null) {
    open = { id: s ? s.id : 'new', kind: 'edit' };
    // A supply carried over without a strength usually matches the usual dose (10 mg of
    // escitalopram as one 10 mg tablet) — offered, and only saved with the rest.
    const guess = !inTablets && supplies.length <= 1 && med.default_dose > 0 ? num(med.default_dose) : '';
    set = {
      strength: s ? num(s.strength) || guess : '',
      label: s?.label ?? '',
      onHand: s ? fmtCount(s.on_hand) : '',
      boxSize: num(s?.box_size),
      boxes: num(s?.boxes_per_fill ?? 1),
      warnDays: num(s?.warn_days ?? 7),
      repeats: num(s?.repeats_left),
      needsScript: s?.needs_script ?? false,
      expiry: s?.script_expiry ?? '',
    };
    onHandStart = set.onHand;
  }
  async function saveEdit() {
    if (!open) return;
    const isNew = open.id === 'new';
    try {
      await invoke('save_medication_supply', {
        supply: {
          id: isNew ? null : open.id,
          medication_id: med.id,
          strength: parse(set.strength),
          label: set.label || null,
          // Only a changed count resets it (as of now); otherwise it keeps running.
          on_hand: isNew || set.onHand !== onHandStart ? parse(set.onHand) ?? 0 : null,
          box_size: parse(set.boxSize),
          boxes_per_fill: parse(set.boxes) ?? 1,
          warn_days: Math.round(parse(set.warnDays) ?? 7),
          repeats_left: intOrNull(set.repeats),
          needs_script: set.needsScript,
          script_expiry: set.expiry || null,
        },
      });
      showToast(isNew ? 'Supply added' : 'Supply saved');
      open = null;
      await onchange();
    } catch (e) {
      showToast(String(e), 'error');
    }
  }
  async function removeSupply(s: Supply) {
    const ok = await confirmAction({
      title: `Stop tracking ${name(s)}?`,
      message: 'Its count and fills are removed. Logged doses stay as they are.',
      confirmLabel: 'Remove',
    });
    if (!ok) return;
    try {
      await invoke('delete_medication_supply', { id: s.id });
      open = null;
      await Promise.all([onchange(), loadFills()]);
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  async function undoFill(f: Fill) {
    try {
      await invoke('delete_medication_fill', { id: f.id });
      showToast('Fill removed');
      await Promise.all([onchange(), loadFills()]);
    } catch (e) {
      showToast(String(e), 'error');
    }
  }
  // Undo is offered on each supply's latest fill.
  let latestFill = $derived(new Set(supplies.map((s: Supply) => fills.find((f) => f.supply_id === s.id)?.id).filter(Boolean)));
  function fillSupply(f: Fill): string {
    const s = supplies.find((x: Supply) => x.id === f.supply_id);
    return s && supplies.length > 1 ? `${name(s)} · ` : '';
  }

  function usageText(s: Supply): string {
    if (s.per_day == null) return 'Not enough doses logged yet to tell how fast it goes.';
    const a = `about ${fmtCount(s.per_day)} a day`;
    return s.usage_source === 'doses'
      ? `Using ${a}, from the doses logged over the last ${s.usage_days} days.`
      : `Using ${a} going by the quick-add buttons, until a few days of doses are logged.`;
  }
</script>

{#snippet editForm()}
  <div class="form">
    <div class="row">
      {#if !inTablets}
        <label>Tablet strength <input class="n" bind:value={set.strength} inputmode="decimal" placeholder="—" /> {unit}</label>
      {/if}
      <label>Label <input class="w" bind:value={set.label} placeholder="Optional, e.g. brand" /></label>
      <label>On hand now <input class="n" bind:value={set.onHand} placeholder="0" inputmode="decimal" /></label>
    </div>
    <div class="row">
      <label>Tablets per box <input class="n" bind:value={set.boxSize} inputmode="decimal" /></label>
      <label>Boxes per fill <input class="n" bind:value={set.boxes} inputmode="decimal" /></label>
      <label>Warn at <input class="n" bind:value={set.warnDays} inputmode="numeric" /> days left</label>
    </div>
    <div class="row">
      <label>Repeats left <input class="n" bind:value={set.repeats} placeholder="—" inputmode="numeric" /></label>
      <label>Script expires <input type="date" bind:value={set.expiry} /></label>
      <label class="check"><input type="checkbox" bind:checked={set.needsScript} /> Need a new script</label>
    </div>
    <div class="row end">
      {#if open?.id !== 'new'}
        {@const s = supplies.find((x: Supply) => x.id === open?.id)}
        {#if s}<button class="danger" onclick={() => removeSupply(s)}>Stop tracking</button>{/if}
      {/if}
      <button class="cancel" onclick={() => open = null}>Cancel</button>
      <button class="save" onclick={saveEdit}>{open?.id === 'new' ? 'Add' : 'Save'}</button>
    </div>
  </div>
{/snippet}

<div class="supply">
  <div class="supply-head">
    <span class="supply-title">Supply &amp; scripts</span>
    <button class="cancel close" onclick={onclose}>Close</button>
  </div>

  {#if supplies.length === 0 && open?.id !== 'new'}
    <p class="hint">
      Not tracked yet. Add the tablets you have{inTablets ? '' : ' — one per strength if you take more than one (an 8 mg and a 4 mg tablet for 12 mg)'}
      and logged doses count them down.
    </p>
  {/if}

  {#each supplies as s (s.id)}
    {@const lvl = supplyLevel(s)}
    <div class="item">
      <div class="item-head">
        <span class="item-name">{name(s)}</span>
        <span class="item-count">
          {fmtCount(s.on_hand)} on hand{#if s.days_left != null} · <span class="days {lvl}">{s.days_left <= 0 ? 'run out' : `${s.days_left} day${s.days_left === 1 ? '' : 's'} left`}</span>{/if}
        </span>
        <span class="item-actions">
          <button class="pill" class:on={isOpen(s.id, 'fill')} onclick={() => isOpen(s.id, 'fill') ? open = null : openFill(s)}>Script filled</button>
          <button class="pill" class:on={isOpen(s.id, 'edit')} onclick={() => isOpen(s.id, 'edit') ? open = null : openEdit(s)}>Settings</button>
        </span>
      </div>
      <div class="item-sub">
        {usageText(s)}
        {#if s.repeats_left != null} {s.repeats_left} repeat{s.repeats_left === 1 ? '' : 's'} left.{/if}
        {#if s.script_expiry} Script expires {formatDate(s.script_expiry)}.{/if}
        {#if s.needs_script}<strong class="flag"> New script needed.</strong>{/if}
      </div>
      {#if s.needs_strength}
        <div class="warn">Set the tablet strength so doses can be counted — until then each dose counts as one tablet.</div>
      {/if}

      {#if isOpen(s.id, 'fill')}
        <div class="form">
          <div class="row">
            <label>Date <input type="date" bind:value={fill.date} max={todayISO()} /></label>
            <label>Boxes <input class="n" bind:value={fill.boxes} inputmode="decimal" /></label>
            <span class="x">×</span>
            <label>tablets per box <input class="n" bind:value={fill.boxSize} inputmode="decimal" /></label>
            <span class="eq">= <strong>{fmtCount(fillQty)}</strong> tablets</span>
          </div>
          <div class="row">
            <label>Repeats left after this <input class="n" bind:value={fill.repeats} oninput={onRepeatsInput} placeholder="—" inputmode="numeric" /></label>
            <label>Script expires <input type="date" bind:value={fill.expiry} /></label>
            <label class="check"><input type="checkbox" bind:checked={fill.needsScript} /> Need a new script</label>
          </div>
          <div class="row">
            <input class="note" bind:value={fill.note} placeholder="Note (optional)" />
            <button class="cancel" onclick={() => open = null}>Cancel</button>
            <button class="save" onclick={() => saveFill(s)} disabled={!(fillQty > 0)}>Add {fmtCount(fillQty)} tablets</button>
          </div>
          {#if !usualFill(s)}<p class="hint">The box size is remembered for next time.</p>{/if}
        </div>
      {:else if isOpen(s.id, 'edit')}
        {@render editForm()}
      {/if}
    </div>
  {/each}

  {#if open?.id === 'new'}
    <div class="item">
      <div class="item-head"><span class="item-name">New {inTablets ? 'supply' : 'strength / script'}</span></div>
      {@render editForm()}
    </div>
  {:else}
    <button class="link add" onclick={() => openEdit(null)}>+ Add {supplies.length ? `another ${inTablets ? 'supply' : 'strength or script'}` : 'tablets'}</button>
  {/if}

  {#if fills.length}
    <div class="fills">
      <span class="fills-label">Fills</span>
      {#each fills.slice(0, 6) as f (f.id)}
        <span class="fill">
          {formatDate(f.fill_date)} · {fillSupply(f)}+{fmtCount(f.quantity)}{#if f.note} · {f.note}{/if}
          {#if latestFill.has(f.id)}<button class="link" onclick={() => undoFill(f)} title="Remove this fill">Undo</button>{/if}
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .supply { display:flex; flex-direction:column; gap:10px; padding:12px 18px 14px; background:var(--inset); border-top:1px solid var(--border); }
  .supply-head { display:flex; align-items:center; gap:12px; }
  .supply-title { font-size:12.5px; font-weight:700; color:var(--tp); }
  .close { margin-left:auto; }
  .item { background:var(--card); border:1px solid var(--border); border-radius:12px; padding:10px 12px; display:flex; flex-direction:column; gap:6px; }
  .item-head { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .item-name { font-size:13px; font-weight:700; color:var(--tp); }
  .item-count { font-size:12px; color:var(--ts); font-variant-numeric:tabular-nums; }
  .days.low { color:var(--amber-fg); font-weight:700; }
  .days.out { color:var(--red-fg); font-weight:700; }
  .item-actions { margin-left:auto; display:flex; gap:6px; }
  .item-sub { font-size:11.5px; color:var(--tm); line-height:1.5; }
  .flag { color:var(--red-fg); }
  .warn { font-size:11.5px; color:var(--amber-fg); background:var(--amber-soft); border-radius:8px; padding:6px 9px; }
  .form { display:flex; flex-direction:column; gap:9px; border-top:1px solid var(--border); padding-top:9px; margin-top:2px; }
  .row { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .row.end { justify-content:flex-end; }
  label { display:flex; align-items:center; gap:6px; font-size:11.5px; color:var(--ts); font-weight:600; }
  input { background:var(--card); border:1px solid var(--border); border-radius:9px; padding:6px 8px; font-size:12.5px; color:var(--tp); }
  input.n { width:58px; text-align:center; font-variant-numeric:tabular-nums; }
  input.w { width:150px; }
  input.note { flex:1; min-width:140px; }
  label.check input { width:auto; }
  .x, .eq { font-size:12px; color:var(--ts); }
  .eq strong { color:var(--tp); }
  .pill { background:var(--inset); border:1px solid var(--border); border-radius:999px; padding:5px 11px; font-size:11.5px; font-weight:600; color:var(--ts); cursor:pointer; }
  .pill.on { background:var(--accent-soft); color:var(--accent-fg); border-color:var(--accent-soft); }
  .save { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:8px 15px; font-size:12px; font-weight:700; cursor:pointer; }
  .save:disabled { opacity:.5; cursor:default; }
  .cancel { background:transparent; border:none; color:var(--ts); font-size:12px; font-weight:600; cursor:pointer; }
  .danger { background:transparent; border:none; color:var(--red-fg); font-size:12px; font-weight:600; cursor:pointer; margin-right:auto; }
  .link { background:none; border:none; padding:0; color:var(--accent-fg); font-size:11.5px; font-weight:600; cursor:pointer; text-decoration:underline; }
  .link.add { align-self:flex-start; font-size:12px; text-decoration:none; }
  .hint { margin:0; font-size:11.5px; color:var(--tm); }
  .fills { display:flex; flex-wrap:wrap; gap:6px 14px; align-items:center; font-size:11.5px; color:var(--ts); border-top:1px solid var(--border); padding-top:8px; }
  .fills-label { font-weight:700; color:var(--tm); text-transform:uppercase; font-size:10.5px; letter-spacing:.04em; }
  .fill { display:inline-flex; gap:6px; align-items:center; }
</style>
