<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { formatDate, formatDateLong, todayISO, shiftISO } from '$lib/formatDate';
  import { dateFromUrl, pushDate } from '$lib/dateParam';
  import { showToast } from '$lib/stores/toast.svelte';
  import { confirmAction } from '$lib/stores/confirm.svelte';
  import { recallView, rememberView, oneOf } from '$lib/viewState';

  // Food & drink log. Not calorie counting: an item is a name and a kind, and the point
  // is to see what was eaten beside how the fatigue went. Like the Medication page:
  // regular items get quick-add buttons, groups log several items at once.

  interface Food { id: number; name: string; kind: 'food' | 'drink'; regular: boolean; active: boolean; days_logged: number; }
  interface Group { id: number; name: string; default_time: string | null; food_ids: number[]; }

  let today = $state(todayISO());
  let selectedDate = $state(dateFromUrl($page.url));
  let foods = $state<Food[]>([]);
  let groups = $state<Group[]>([]);
  let entries = $state<any[]>([]);
  let foodDays = $state<{ food_id: number; log_date: string }[]>([]);
  let fatigueLogs = $state<{ log_date: string; fatigue_rating: number | null }[]>([]);
  let loading = $state(true);

  const saved = recallView<any>('food');
  let showOther = $state(saved.showOther === true);
  let sortBy = $state<'days' | 'next'>(oneOf(saved.sortBy, ['days', 'next'] as const, 'days'));
  $effect(() => rememberView('food', { showOther, sortBy }));

  onMount(async () => {
    try {
      await Promise.all([loadDefinitions(), loadDay(), loadHistory()]);
    } catch (e) {
      console.error('Error loading food data:', e);
    } finally {
      loading = false;
    }
  });

  async function loadDefinitions() {
    [foods, groups] = await Promise.all([
      invoke<Food[]>('list_foods'),
      invoke<Group[]>('list_food_groups'),
    ]);
  }
  async function loadDay() {
    const date = selectedDate;
    const list = await invoke<any[]>('get_food_log_for_date', { date });
    if (date === selectedDate) entries = list;
  }
  async function loadHistory() {
    const [days, logs] = await Promise.all([
      invoke<any[]>('get_food_days'),
      invoke<any[]>('list_daily_logs', { limit: 100000, offset: 0 }),
    ]);
    foodDays = days;
    fatigueLogs = logs.map((l) => ({ log_date: l.log_date, fatigue_rating: l.fatigue_rating }));
  }
  // After anything is logged or removed: the day, the item counts and the comparison all move.
  async function refreshAfterLog() {
    await Promise.all([loadDay(), loadDefinitions(), loadHistory()]);
  }

  function prevDay() { selectedDate = shiftISO(selectedDate, -1); pushDate(selectedDate); closeForms(); loadDay(); }
  function nextDay() { selectedDate = shiftISO(selectedDate, 1); pushDate(selectedDate); closeForms(); loadDay(); }
  function goToday() { selectedDate = today; pushDate(selectedDate); closeForms(); loadDay(); }

  function nowHHMM(): string {
    const d = new Date();
    return String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
  }
  // Logging for a past day shouldn't stamp the entry with this minute — leave the time blank.
  function defaultTime(): string { return selectedDate === today ? nowHHMM() : ''; }

  let regularFoods = $derived(foods.filter((f) => f.active && f.regular && f.kind === 'food'));
  let regularDrinks = $derived(foods.filter((f) => f.active && f.regular && f.kind === 'drink'));
  let otherItems = $derived(foods.filter((f) => !f.active || !f.regular));
  let activeFoods = $derived(foods.filter((f) => f.active));
  function foodName(id: number): string { return foods.find((f) => f.id === id)?.name ?? '?'; }

  // ── Log a single item ──
  let logId = $state<number | null>(null);
  let logForm = $state({ amount: '', time: '' });
  function openLog(f: Food) {
    closeForms();
    logId = f.id;
    logForm = { amount: '', time: defaultTime() };
  }
  async function saveLog(f: Food) {
    try {
      await invoke('add_food_log', {
        logDate: selectedDate,
        entries: [{ food_id: f.id, time_taken: logForm.time || null, amount: logForm.amount || null, group_id: null }],
      });
      logId = null;
      await refreshAfterLog();
      showToast(`${f.name} logged`);
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  // ── Log a group ──
  let groupLogId = $state<number | null>(null);
  let groupLogTime = $state('');
  let groupLogPicks = $state<Record<number, boolean>>({});
  function openGroupLog(g: Group) {
    closeForms();
    groupLogId = g.id;
    groupLogTime = selectedDate === today ? (g.default_time ?? nowHHMM()) : (g.default_time ?? '');
    groupLogPicks = Object.fromEntries(g.food_ids.map((id) => [id, true]));
  }
  async function saveGroupLog(g: Group) {
    const ids = g.food_ids.filter((id) => groupLogPicks[id]);
    if (!ids.length) return;
    try {
      await invoke('add_food_log', {
        logDate: selectedDate,
        entries: ids.map((id) => ({ food_id: id, time_taken: groupLogTime || null, amount: null, group_id: g.id })),
      });
      groupLogId = null;
      await refreshAfterLog();
      showToast(`${g.name}: ${ids.length} item${ids.length === 1 ? '' : 's'} logged`);
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  // ── Log anything by name (creates a one-off item if it's new) ──
  let quick = $state({ name: '', kind: 'food' as 'food' | 'drink', time: '', amount: '' });
  let quickMatch = $derived(foods.find((f) => f.name.toLowerCase() === quick.name.trim().toLowerCase()) ?? null);
  async function quickLog() {
    const name = quick.name.trim();
    if (!name) return;
    try {
      let id = quickMatch?.id;
      if (id == null) id = await invoke<number>('create_food', { name, kind: quick.kind, regular: false });
      await invoke('add_food_log', {
        logDate: selectedDate,
        entries: [{ food_id: id, time_taken: quick.time || null, amount: quick.amount || null, group_id: null }],
      });
      quick = { name: '', kind: quick.kind, time: '', amount: '' };
      await refreshAfterLog();
      showToast(`${name} logged`);
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  async function deleteEntry(id: number) {
    await invoke('delete_food_log', { id });
    await refreshAfterLog();
  }

  // ── Items: add / edit / hide / delete ──
  let showAddItem = $state(false);
  let newItem = $state({ name: '', kind: 'food' as 'food' | 'drink', regular: true });
  async function addItem() {
    if (!newItem.name.trim()) return;
    try {
      await invoke('create_food', { name: newItem.name, kind: newItem.kind, regular: newItem.regular });
      showToast(`${newItem.name.trim()} added`);
      newItem = { name: '', kind: newItem.kind, regular: true };
      showAddItem = false;
      await loadDefinitions();
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  let editId = $state<number | null>(null);
  let edit = $state({ name: '', kind: 'food' as 'food' | 'drink', regular: true });
  function startEdit(f: Food) {
    closeForms();
    editId = f.id;
    edit = { name: f.name, kind: f.kind, regular: f.regular };
  }
  async function saveEdit(f: Food) {
    try {
      await invoke('update_food', { id: f.id, name: edit.name, kind: edit.kind, regular: edit.regular, active: f.active });
      editId = null;
      await Promise.all([loadDefinitions(), loadDay()]);
    } catch (e) {
      showToast(String(e), 'error');
    }
  }
  async function setActive(f: Food, active: boolean) {
    await invoke('update_food', { id: f.id, name: f.name, kind: f.kind, regular: f.regular, active });
    await loadDefinitions();
    showToast(active ? `${f.name} restored` : `${f.name} hidden`);
  }
  async function removeItem(f: Food) {
    const ok = await confirmAction({ title: `Delete ${f.name}?`, message: 'It will be removed from the list and from any groups.', confirmLabel: 'Delete' });
    if (!ok) return;
    try {
      await invoke('delete_food', { id: f.id });
      await loadDefinitions();
    } catch (e) {
      showToast(String(e), 'error');
    }
  }

  // ── Groups: create / edit / delete ──
  let groupFormId = $state<number | 'new' | null>(null);
  let groupForm = $state({ name: '', time: '', picks: {} as Record<number, boolean> });
  function startGroupForm(g: Group | null) {
    closeForms();
    groupFormId = g ? g.id : 'new';
    groupForm = {
      name: g?.name ?? '',
      time: g?.default_time ?? '',
      picks: Object.fromEntries((g?.food_ids ?? []).map((id) => [id, true])),
    };
  }
  async function saveGroupForm() {
    const ids = Object.keys(groupForm.picks).map(Number).filter((id) => groupForm.picks[id]);
    if (!groupForm.name.trim() || !ids.length) return;
    try {
      await invoke('save_food_group', {
        id: groupFormId === 'new' ? null : groupFormId,
        name: groupForm.name,
        defaultTime: groupForm.time || null,
        foodIds: ids,
      });
      groupFormId = null;
      await loadDefinitions();
      showToast('Group saved');
    } catch (e) {
      showToast(String(e), 'error');
    }
  }
  async function removeGroup(g: Group) {
    const ok = await confirmAction({ title: `Delete ${g.name}?`, message: 'Items already logged with this group stay in the log.', confirmLabel: 'Delete' });
    if (!ok) return;
    await invoke('delete_food_group', { id: g.id });
    await Promise.all([loadDefinitions(), loadDay()]);
  }

  // One inline form at a time.
  function closeForms() { logId = null; groupLogId = null; editId = null; groupFormId = null; openMenuId = null; }
  let openMenuId = $state<number | null>(null);

  // ── Fatigue alongside each item ──
  // Descriptive only (see CLAUDE.md: describe, don't forecast). For each item: the mean
  // fatigue rating on days it was had and on the day after, beside the same means over the
  // other days food was logged. "Tracked days" is the baseline so the months before this
  // log began don't count as days without the item.
  const MIN_DAYS = 3;
  function mean(xs: number[]): number | null {
    return xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : null;
  }
  let comparison = $derived.by(() => {
    const fatigue = new Map<string, number>();
    for (const l of fatigueLogs) if (l.fatigue_rating != null) fatigue.set(l.log_date, l.fatigue_rating);
    const tracked = new Set(foodDays.map((d) => d.log_date));
    const byFood = new Map<number, Set<string>>();
    for (const d of foodDays) (byFood.get(d.food_id) ?? byFood.set(d.food_id, new Set()).get(d.food_id)!).add(d.log_date);

    const values = (days: Iterable<string>, shift: number) => {
      const out: number[] = [];
      for (const d of days) {
        const v = fatigue.get(shift ? shiftISO(d, shift) : d);
        if (v != null) out.push(v);
      }
      return out;
    };
    const rows = [];
    for (const [foodId, days] of byFood) {
      const same = values(days, 0);
      if (same.length < MIN_DAYS) continue;
      const without = [...tracked].filter((d) => !days.has(d));
      const next = values(days, 1);
      const baseSame = values(without, 0);
      const baseNext = values(without, 1);
      const sameAvg = mean(same), nextAvg = mean(next);
      const baseSameAvg = baseSame.length >= MIN_DAYS ? mean(baseSame) : null;
      const baseNextAvg = baseNext.length >= MIN_DAYS ? mean(baseNext) : null;
      rows.push({
        foodId,
        name: foodName(foodId),
        days: days.size,
        sameAvg,
        nextAvg,
        sameDiff: sameAvg != null && baseSameAvg != null ? sameAvg - baseSameAvg : null,
        nextDiff: nextAvg != null && baseNextAvg != null ? nextAvg - baseNextAvg : null,
      });
    }
    rows.sort((a, b) => sortBy === 'days'
      ? b.days - a.days
      : (b.nextDiff ?? -Infinity) - (a.nextDiff ?? -Infinity));
    return { rows, trackedDays: tracked.size, hidden: byFood.size - rows.length };
  });
  function fmt(v: number | null): string { return v == null ? '—' : v.toFixed(1); }
  function fmtDiff(v: number | null): string { return v == null ? '—' : (v > 0 ? '+' : v < 0 ? '−' : '±') + Math.abs(v).toFixed(1); }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Food &amp; Drink</div>
    <div class="page-subtitle">What you had each day, set beside how the fatigue went</div>
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
      {#if selectedDate !== today}
        <button class="today-btn" onclick={goToday}>Today</button>
      {/if}
    </div>
    <button class="ghost-btn" onclick={() => startGroupForm(null)}>New group</button>
    <button class="primary-btn" onclick={() => { closeForms(); showAddItem = !showAddItem; }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      Add item
    </button>
  </div>
</div>

{#if showAddItem}
  <div class="form-card">
    <div class="card-heading">New item</div>
    <div class="form-row">
      <div class="text-field grow">
        <label for="ni-name">Name</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="ni-name" bind:value={newItem.name} placeholder="e.g. Toast, Coffee" autofocus onkeydown={(e) => { if (e.key === 'Enter') addItem(); }} />
      </div>
      <div class="seg-control" role="radiogroup" aria-label="Kind">
        <button class="seg-btn" class:active={newItem.kind === 'food'} onclick={() => newItem.kind = 'food'}>Food</button>
        <button class="seg-btn" class:active={newItem.kind === 'drink'} onclick={() => newItem.kind = 'drink'}>Drink</button>
      </div>
      <label class="check"><input type="checkbox" bind:checked={newItem.regular} /> Quick-add button</label>
      <button class="primary-btn" onclick={addItem} disabled={!newItem.name.trim()}>Save</button>
      <button class="ghost-btn" onclick={() => showAddItem = false}>Cancel</button>
    </div>
  </div>
{/if}

{#if groupFormId !== null}
  <div class="form-card">
    <div class="card-heading">{groupFormId === 'new' ? 'New group' : 'Edit group'}</div>
    <div class="form-row">
      <div class="text-field grow">
        <label for="gf-name">Name</label>
        <input id="gf-name" bind:value={groupForm.name} placeholder="e.g. Usual breakfast" />
      </div>
      <div class="text-field">
        <label for="gf-time">Usual time</label>
        <input id="gf-time" type="time" bind:value={groupForm.time} />
      </div>
    </div>
    {#if activeFoods.length === 0}
      <p class="muted">Add some items first, then group them.</p>
    {:else}
      <div class="pick-grid">
        {#each activeFoods as f (f.id)}
          <label class="pick" class:on={groupForm.picks[f.id]}>
            <input type="checkbox" bind:checked={groupForm.picks[f.id]} />
            {f.name}
          </label>
        {/each}
      </div>
    {/if}
    <div class="form-row end">
      <button class="ghost-btn" onclick={() => groupFormId = null}>Cancel</button>
      <button class="primary-btn" onclick={saveGroupForm}
        disabled={!groupForm.name.trim() || !Object.values(groupForm.picks).some(Boolean)}>Save group</button>
    </div>
  </div>
{/if}

{#if loading}
  <p class="muted center">Loading…</p>
{:else}
  {#if openMenuId !== null}
    <button class="menu-backdrop" onclick={() => openMenuId = null} aria-label="Close menu" tabindex="-1"></button>
  {/if}

  {#snippet itemRow(f: Food)}
    {#if editId === f.id}
      <div class="row-edit">
        <input class="edit-name" bind:value={edit.name} aria-label="Name" />
        <div class="seg-control sm">
          <button class="seg-btn" class:active={edit.kind === 'food'} onclick={() => edit.kind = 'food'}>Food</button>
          <button class="seg-btn" class:active={edit.kind === 'drink'} onclick={() => edit.kind = 'drink'}>Drink</button>
        </div>
        <label class="check"><input type="checkbox" bind:checked={edit.regular} /> Quick-add</label>
        <button class="save-sm" onclick={() => saveEdit(f)}>Save</button>
        <button class="cancel-sm" onclick={() => editId = null}>Cancel</button>
      </div>
    {:else}
      <div class="item-row" class:dimmed={!f.active} class:menu-open={openMenuId === f.id}>
        <div class="item-info">
          <span class="item-name" class:drink={f.kind === 'drink'}>{f.name}</span>
          <span class="item-detail">
            {#if !f.active}hidden · {/if}{#if !f.regular && f.active}{f.kind} · {/if}{f.days_logged ? `${f.days_logged} day${f.days_logged === 1 ? '' : 's'}` : 'not logged yet'}
          </span>
        </div>
        {#if f.active}
          <button class="add-dose-btn" onclick={() => openLog(f)} aria-label="Log {f.name}" title="Log {f.name}">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
          </button>
        {/if}
        <div class="row-menu">
          <button class="icon-btn" onclick={() => openMenuId = openMenuId === f.id ? null : f.id} aria-label="More actions" aria-haspopup="menu" aria-expanded={openMenuId === f.id}>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="5" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="12" cy="19" r="1.7"/></svg>
          </button>
          {#if openMenuId === f.id}
            <div class="menu-pop" role="menu">
              <button class="menu-item" role="menuitem" onclick={() => startEdit(f)}>Edit</button>
              <button class="menu-item" role="menuitem" onclick={() => { openMenuId = null; setActive(f, !f.active); }}>{f.active ? 'Hide' : 'Restore'}</button>
              {#if f.days_logged === 0}
                <button class="menu-item danger" role="menuitem" onclick={() => { openMenuId = null; removeItem(f); }}>Delete</button>
              {/if}
            </div>
          {/if}
        </div>
      </div>
      {#if logId === f.id}
        <div class="inline-form">
          <span class="lbl">Amount</span>
          <input class="sm-input wide" bind:value={logForm.amount} placeholder="optional" />
          <span class="lbl">at</span>
          <input class="sm-input" type="time" bind:value={logForm.time} />
          <button class="save-sm" onclick={() => saveLog(f)}>Log</button>
          <button class="cancel-sm" onclick={() => logId = null}>Cancel</button>
        </div>
      {/if}
    {/if}
  {/snippet}

  <div class="layout">
    <div class="list-card">
      <div class="section-divider">Groups</div>
      {#if groups.length === 0}
        <div class="section-empty">No groups yet &mdash; use <strong>New group</strong> to log a usual meal in one go.</div>
      {/if}
      {#each groups as g (g.id)}
        <div class="item-row" class:menu-open={openMenuId === -g.id}>
          <div class="item-info">
            <span class="item-name group">{g.name}</span>
            <span class="item-detail">{g.food_ids.map(foodName).join(', ')}{g.default_time ? ` · ${g.default_time}` : ''}</span>
          </div>
          <button class="slot-btn" onclick={() => openGroupLog(g)}>Log</button>
          <div class="row-menu">
            <button class="icon-btn" onclick={() => openMenuId = openMenuId === -g.id ? null : -g.id} aria-label="More actions" aria-haspopup="menu" aria-expanded={openMenuId === -g.id}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="5" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="12" cy="19" r="1.7"/></svg>
            </button>
            {#if openMenuId === -g.id}
              <div class="menu-pop" role="menu">
                <button class="menu-item" role="menuitem" onclick={() => startGroupForm(g)}>Edit</button>
                <button class="menu-item danger" role="menuitem" onclick={() => { openMenuId = null; removeGroup(g); }}>Delete</button>
              </div>
            {/if}
          </div>
        </div>
        {#if groupLogId === g.id}
          <div class="inline-form col">
            {#each g.food_ids as id}
              <label class="check"><input type="checkbox" bind:checked={groupLogPicks[id]} /> {foodName(id)}</label>
            {/each}
            <div class="inline-form-row">
              <span class="lbl">Time</span>
              <input class="sm-input" type="time" bind:value={groupLogTime} />
              <button class="save-sm" onclick={() => saveGroupLog(g)} disabled={!Object.values(groupLogPicks).some(Boolean)}>Log these</button>
              <button class="cancel-sm" onclick={() => groupLogId = null}>Cancel</button>
            </div>
          </div>
        {/if}
      {/each}

      <div class="section-divider">Food</div>
      {#if regularFoods.length === 0}<div class="section-empty">No regular foods yet.</div>{/if}
      {#each regularFoods as f (f.id)}{@render itemRow(f)}{/each}

      <div class="section-divider">Drinks</div>
      {#if regularDrinks.length === 0}<div class="section-empty">No regular drinks yet.</div>{/if}
      {#each regularDrinks as f (f.id)}{@render itemRow(f)}{/each}

      {#if otherItems.length}
        <button class="other-toggle" class:open={showOther} onclick={() => showOther = !showOther} aria-expanded={showOther}>
          <svg class="chev" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>
          One-off &amp; hidden items
          <span class="count">{otherItems.length}</span>
        </button>
        {#if showOther}
          {#each otherItems as f (f.id)}{@render itemRow(f)}{/each}
        {/if}
      {/if}
    </div>

    <div class="right-col">
      <div class="day-card">
        <div class="day-header">
          <span class="card-heading">Had today</span>
          <span class="day-date">{formatDate(selectedDate)}</span>
        </div>
        <div class="quick">
          <input class="quick-name" list="food-names" placeholder="Log anything…" bind:value={quick.name}
            onkeydown={(e) => { if (e.key === 'Enter') quickLog(); }} aria-label="Item" />
          <datalist id="food-names">
            {#each activeFoods as f}<option value={f.name}></option>{/each}
          </datalist>
          <input class="sm-input" type="time" bind:value={quick.time} aria-label="Time" />
          <button class="save-sm" onclick={quickLog} disabled={!quick.name.trim()}>Log</button>
        </div>
        {#if quick.name.trim() && !quickMatch}
          <div class="quick-new">
            New item &mdash; save as
            <div class="seg-control sm">
              <button class="seg-btn" class:active={quick.kind === 'food'} onclick={() => quick.kind = 'food'}>Food</button>
              <button class="seg-btn" class:active={quick.kind === 'drink'} onclick={() => quick.kind = 'drink'}>Drink</button>
            </div>
          </div>
        {/if}
        {#if entries.length === 0}
          <p class="muted center">Nothing logged</p>
        {:else}
          {#each entries as e (e.id)}
            <div class="entry" class:drink={e.kind === 'drink'}>
              <span class="entry-time">{e.time_taken ?? '--:--'}</span>
              <div class="entry-name">
                {e.food_name}
                {#if e.amount}<span class="entry-amt">{e.amount}</span>{/if}
                {#if e.group_name}<span class="entry-group">{e.group_name}</span>{/if}
              </div>
              <button class="entry-del" onclick={() => deleteEntry(e.id)} aria-label="Remove {e.food_name}">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
              </button>
            </div>
          {/each}
        {/if}
        <div class="day-footer">{entries.length} item{entries.length === 1 ? '' : 's'} logged</div>
      </div>
    </div>
  </div>

  <div class="compare-card">
    <div class="compare-head">
      <div>
        <div class="card-heading">Fatigue alongside each item</div>
        <div class="card-subtitle">
          Average fatigue rating on the days you had it, and the day after, compared with the other
          {comparison.trackedDays} day{comparison.trackedDays === 1 ? '' : 's'} you logged food.
        </div>
      </div>
      <div class="seg-control sm">
        <button class="seg-btn" class:active={sortBy === 'days'} onclick={() => sortBy = 'days'}>Most logged</button>
        <button class="seg-btn" class:active={sortBy === 'next'} onclick={() => sortBy = 'next'}>Largest next-day gap</button>
      </div>
    </div>
    {#if comparison.rows.length === 0}
      <p class="muted">Items appear here once they've been logged on at least {MIN_DAYS} days that also have a fatigue rating.</p>
    {:else}
      <div class="cmp-grid cmp-head">
        <span>Item</span><span>Days</span><span>Same day</span><span>vs other days</span><span>Day after</span><span>vs other days</span>
      </div>
      {#each comparison.rows as r (r.foodId)}
        <div class="cmp-grid cmp-row">
          <span class="cmp-name">{r.name}</span>
          <span class="num">{r.days}</span>
          <span class="num">{fmt(r.sameAvg)}</span>
          <span class="num diff" class:up={(r.sameDiff ?? 0) >= 0.5} class:down={(r.sameDiff ?? 0) <= -0.5}>{fmtDiff(r.sameDiff)}</span>
          <span class="num">{fmt(r.nextAvg)}</span>
          <span class="num diff" class:up={(r.nextDiff ?? 0) >= 0.5} class:down={(r.nextDiff ?? 0) <= -0.5}>{fmtDiff(r.nextDiff)}</span>
        </div>
      {/each}
    {/if}
    <div class="cmp-note">
      A record, not a verdict: higher = more fatigued (0&ndash;10). A few days either way can swing
      these averages a long way, and a bad day can change what you eat as easily as the other way
      round &mdash; treat a gap as something to keep an eye on.
      {#if comparison.hidden}{comparison.hidden} item{comparison.hidden === 1 ? ' is' : 's are'} hidden until logged on {MIN_DAYS}+ rated days.{/if}
    </div>
  </div>
{/if}

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:22px; gap:16px; flex-wrap:wrap; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }
  .header-actions { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .day-nav { display:flex; align-items:center; gap:2px; background:var(--card); border:1px solid var(--border); border-radius:999px; padding:4px; box-shadow:var(--shadow); }
  .day-arrow { width:30px;height:30px;border-radius:50%;border:none;background:transparent;color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .day-arrow:disabled { color:var(--tm); cursor:not-allowed; }
  .day-label { font-weight:700; font-size:13px; padding:0 6px; min-width:108px; text-align:center; }
  .today-btn { background:var(--accent-soft);color:var(--accent-fg);border:none;border-radius:999px;padding:6px 12px;font-size:12px;font-weight:700;cursor:pointer;margin-left:4px; }
  .primary-btn { display:inline-flex;align-items:center;gap:7px;background:var(--accent);color:#fff;border:none;border-radius:999px;padding:10px 16px;font-size:13px;font-weight:700;cursor:pointer; }
  .primary-btn:disabled { opacity:.5; cursor:not-allowed; }
  .ghost-btn { background:var(--card);color:var(--ts);border:1px solid var(--border);border-radius:999px;padding:10px 16px;font-size:13px;font-weight:700;cursor:pointer; }
  .ghost-btn:hover { background:var(--inset); }

  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .card-subtitle { font-size:12.5px; color:var(--ts); margin-top:2px; line-height:1.45; }
  .muted { color:var(--ts); font-size:13px; }
  .center { text-align:center; padding:22px; }

  .form-card { background:var(--card);border:1px solid var(--border);border-radius:18px;padding:20px;box-shadow:var(--shadow);margin-bottom:16px;display:flex;flex-direction:column;gap:14px; }
  .form-row { display:flex; gap:12px; flex-wrap:wrap; align-items:end; }
  .form-row.end { justify-content:flex-end; }
  .text-field { display:flex; flex-direction:column; gap:6px; }
  .text-field.grow { flex:1; min-width:180px; }
  .text-field label { font-size:12px; font-weight:700; color:var(--ts); }
  .text-field input { background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:10px 12px; font-size:13.5px; color:var(--tp); }
  .seg-control { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px; gap:2px; }
  .seg-control.sm { padding:2px; border-radius:10px; }
  .seg-btn { background:transparent; border:none; border-radius:9px; padding:8px 12px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); }
  .seg-control.sm .seg-btn { padding:6px 10px; font-size:12px; }
  .seg-btn.active { background:var(--accent); color:#fff; }
  .check { display:inline-flex; align-items:center; gap:7px; font-size:12.5px; font-weight:600; color:var(--ts); cursor:pointer; }
  .pick-grid { display:flex; flex-wrap:wrap; gap:7px; }
  .pick { display:inline-flex; align-items:center; gap:7px; padding:6px 11px; border:1px solid var(--border); border-radius:999px; font-size:12.5px; font-weight:600; color:var(--ts); cursor:pointer; background:var(--inset); }
  .pick.on { background:var(--accent-soft); color:var(--accent-fg); border-color:var(--accent-soft); }
  .pick input { accent-color:var(--accent); }

  .layout { display:grid; grid-template-columns:1.6fr 1fr; gap:16px; align-items:start; }
  .list-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .section-divider { font-size:10.5px; letter-spacing:.07em; text-transform:uppercase; font-weight:800; color:var(--tm); border-top:1px solid var(--border); padding:10px 18px 6px; }
  .section-divider:first-child { border-top:none; }
  .section-empty { font-size:12.5px; color:var(--tm); padding:4px 18px 12px; }

  .item-row { display:flex; align-items:center; gap:10px; padding:9px 18px; border-top:1px solid var(--border); }
  .item-row.dimmed { opacity:.6; }
  .item-row.menu-open { position:relative; z-index:51; opacity:1; }
  .item-info { flex:1; min-width:0; display:flex; align-items:baseline; gap:9px; flex-wrap:wrap; }
  .item-name { font-size:13.5px; font-weight:600; color:var(--tp); padding:3px 9px; border-radius:7px; background:var(--lime-soft); }
  .item-name.drink { background:var(--sky-soft); }
  .item-name.group { background:var(--amber-soft); }
  .item-detail { font-size:11.5px; color:var(--tm); }
  .slot-btn { background:var(--accent-soft); color:var(--accent-fg); border:1px solid var(--border); border-radius:999px; padding:6px 13px; font-size:11.5px; font-weight:700; cursor:pointer; }
  .add-dose-btn { display:inline-flex;align-items:center;justify-content:center;width:28px;height:28px;background:var(--card);color:var(--accent-fg);border:2px solid var(--accent-fg);border-radius:999px;padding:0;cursor:pointer;flex-shrink:0; }
  .add-dose-btn:hover { background:var(--accent-soft); }
  .icon-btn { width:30px;height:28px;border-radius:8px;border:1px solid var(--border);background:var(--card);color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .icon-btn:hover { background:var(--inset); }
  .row-menu { position:relative; flex-shrink:0; }
  .menu-backdrop { position:fixed; inset:0; z-index:40; background:transparent; border:none; cursor:default; padding:0; }
  .menu-pop { position:absolute; top:34px; right:0; z-index:50; min-width:130px; background:var(--card); border:1px solid var(--border); border-radius:12px; box-shadow:0 8px 24px rgba(0,0,0,.14); padding:5px; display:flex; flex-direction:column; gap:1px; }
  .menu-item { width:100%; background:transparent; border:none; border-radius:8px; padding:9px 11px; font-size:13px; font-weight:600; color:var(--tp); cursor:pointer; text-align:left; }
  .menu-item:hover { background:var(--inset); }
  .menu-item.danger { color:var(--red-fg); }

  .row-edit, .inline-form { display:flex; align-items:center; gap:9px; padding:12px 18px; border-top:1px solid var(--border); background:var(--inset); flex-wrap:wrap; }
  .inline-form.col { flex-direction:column; align-items:flex-start; }
  .inline-form-row { display:flex; align-items:center; gap:9px; width:100%; padding-top:6px; }
  .edit-name { flex:1; min-width:120px; background:var(--card); border:1px solid var(--border); border-radius:9px; padding:8px; font-size:13px; color:var(--tp); }
  .lbl { font-size:11.5px; color:var(--ts); font-weight:600; }
  .sm-input { width:96px; background:var(--card); border:1px solid var(--border); border-radius:9px; padding:7px; font-size:12.5px; color:var(--tp); }
  .sm-input.wide { width:130px; }
  .save-sm { background:var(--accent);color:#fff;border:none;border-radius:999px;padding:8px 15px;font-size:12px;font-weight:700;cursor:pointer; }
  .save-sm:disabled { opacity:.5; cursor:not-allowed; }
  .inline-form-row .save-sm { margin-left:auto; }
  .cancel-sm { background:transparent;border:none;color:var(--ts);font-size:12px;font-weight:600;cursor:pointer; }

  .other-toggle { display:flex; align-items:center; gap:8px; width:100%; padding:11px 18px; border:none; border-top:1px solid var(--border); background:transparent; cursor:pointer; font-size:10.5px; letter-spacing:.07em; text-transform:uppercase; font-weight:800; color:var(--tm); text-align:left; }
  .other-toggle:hover { background:var(--inset); }
  .chev { transition:transform .15s; }
  .other-toggle.open .chev { transform:rotate(90deg); }
  .count { margin-left:auto; font-size:11px; letter-spacing:0; background:var(--inset); border-radius:999px; padding:2px 8px; }

  .right-col { display:flex; flex-direction:column; gap:16px; }
  .day-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .day-header { display:flex; justify-content:space-between; align-items:center; padding:16px 18px 12px; }
  .day-date { font-size:11.5px; color:var(--tm); }
  .quick { display:flex; gap:8px; padding:0 18px 12px; }
  .quick-name { flex:1; min-width:0; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:8px 11px; font-size:13px; color:var(--tp); }
  .quick-new { display:flex; align-items:center; gap:9px; padding:0 18px 12px; font-size:12px; color:var(--ts); font-weight:600; }
  .entry { display:flex; align-items:center; gap:11px; padding:10px 18px; border-top:1px solid var(--border); }
  .entry-time { font-size:12px; color:var(--ts); font-variant-numeric:tabular-nums; width:42px; font-weight:600; flex-shrink:0; }
  .entry-name { flex:1; min-width:0; font-size:13px; color:var(--tp); display:flex; align-items:baseline; gap:7px; flex-wrap:wrap; }
  .entry.drink .entry-name::before { content:''; width:7px; height:7px; border-radius:50%; background:var(--sky); align-self:center; }
  .entry-amt { font-size:12px; color:var(--tm); }
  .entry-group { font-size:10.5px; font-weight:700; color:var(--amber-fg); background:var(--amber-soft); border-radius:999px; padding:2px 8px; }
  .entry-del { width:24px;height:24px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer;flex-shrink:0; }
  .day-footer { padding:12px 18px; border-top:1px solid var(--border); font-size:12px; color:var(--tm); }

  .compare-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); margin-top:16px; display:flex; flex-direction:column; gap:10px; }
  .compare-head { display:flex; justify-content:space-between; align-items:flex-start; gap:16px; flex-wrap:wrap; margin-bottom:6px; }
  .cmp-grid { display:grid; grid-template-columns:minmax(140px,1fr) 60px 90px 110px 90px 110px; gap:10px; align-items:center; }
  .cmp-head span { font-size:10px; letter-spacing:.05em; text-transform:uppercase; font-weight:800; color:var(--tm); }
  .cmp-head span:not(:first-child) { text-align:right; }
  .cmp-row { padding:8px 0; border-top:1px solid var(--border); }
  .cmp-name { font-size:13px; font-weight:600; color:var(--tp); }
  .num { text-align:right; font-size:13px; color:var(--ts); font-variant-numeric:tabular-nums; }
  .diff.up { color:var(--red-fg); font-weight:700; }
  .diff.down { color:var(--accent-fg); font-weight:700; }
  .cmp-note { font-size:11.5px; color:var(--tm); line-height:1.55; margin-top:6px; max-width:90ch; }
</style>
