<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { formatDate } from '$lib/formatDate';

  let medications = $state<any[]>([]);
  let dosing: Record<number, { amount: string; time: string }> = $state({});
  let openId = $state<number | null>(null);
  let history = $state<any[]>([]);
  let todayDoses = $state<any[]>([]);
  let loading = $state(true);
  let darkMode = $state(false);
  let banner = $state(false);
  let bannerText = $state('');

  let showAddMed = $state(false);
  let nmName = $state('');
  let nmDose = $state('');
  let nmUnit = $state('mg');
  let nmType = $state('regular');

  let today = $state(new Date().toISOString().split('T')[0]);

  onMount(async () => {
    try {
      const [meds, hist, doses] = await Promise.all([
        invoke<any[]>('list_medications'),
        invoke<any[]>('get_medication_history', { medication_id: null }),
        invoke<any[]>('get_doses_for_date', { date: today }),
      ]);
      medications = meds;
      history = hist;
      todayDoses = doses;
    } catch (e) {
      console.error('Error loading meds:', e);
    } finally {
      loading = false;
    }
  });

  function toggleTheme() {
    darkMode = !darkMode;
    document.documentElement.classList.toggle('dark', darkMode);
  }

  async function addMedication() {
    if (!nmName.trim()) return;
    const med = await invoke('create_medication', {
      name: nmName.trim(),
      shortCode: null,
      defaultDose: nmDose ? parseFloat(nmDose) : null,
      doseUnit: nmUnit || 'mg',
      category: nmType === 'prn' ? 'PRN' : null,
    });
    medications = [...medications, med];
    const h = await invoke<any[]>('get_medication_history', { medication_id: null });
    history = h;
    bannerText = `History updated — ${nmName} started.`;
    banner = true;
    setTimeout(() => banner = false, 4000);
    nmName = ''; nmDose = ''; nmUnit = 'mg'; nmType = 'regular';
    showAddMed = false;
  }

  async function toggleActive(med: any) {
    const nowActive = !med.active;
    await invoke('update_medication', {
      id: med.id,
      active: nowActive,
    });
    medications = medications.map((m: any) => m.id === med.id ? { ...m, active: nowActive } : m);
    const h = await invoke<any[]>('get_medication_history', { medication_id: null });
    history = h;
    bannerText = `History updated — ${med.name} marked ${nowActive ? 'restarted' : 'ceased'}.`;
    banner = true;
    setTimeout(() => banner = false, 4000);
  }

  function openDoseForm(medId: number) {
    const med = medications.find((m: any) => m.id === medId);
    if (!med) return;
    openId = medId;
    const now = new Date();
    const timeStr = String(now.getHours()).padStart(2, '0') + ':' + String(now.getMinutes()).padStart(2, '0');
    dosing[medId] = {
      amount: med.default_dose != null ? String(med.default_dose) : '',
      time: timeStr,
    };
  }

  function cancelDose() {
    openId = null;
  }

  async function saveDose(medId: number) {
    const d = dosing[medId];
    if (!d) return;
    await invoke('upsert_dose', {
      dose: {
        medication_id: medId,
        log_date: today,
        time_taken: d.time || null,
        dose_amount: d.amount ? parseFloat(d.amount) : null,
        notes: null,
      },
    });
    openId = null;
    todayDoses = await invoke('get_doses_for_date', { date: today });
  }

  async function deleteDose(medId: number, time: string) {
    await invoke('upsert_dose', {
      dose: { medication_id: medId, log_date: today, time_taken: time, dose_amount: null, notes: null },
    });
    todayDoses = await invoke('get_doses_for_date', { date: today });
  }

  function getMedName(medId: number): string {
    return medications.find((m: any) => m.id === medId)?.name ?? '';
  }

  function getMedUnit(medId: number): string {
    return medications.find((m: any) => m.id === medId)?.dose_unit ?? 'mg';
  }

  let regularMeds = $derived(medications.filter((m: any) => !m.category || m.category !== 'PRN'));
  let prnMeds = $derived(medications.filter((m: any) => m.category === 'PRN'));

  let eventMeta: Record<string, { label: string; style: string }> = {
    started: { label: 'Started', style: 'color:var(--accent-fg);background:var(--accent-soft);' },
    ceased: { label: 'Ceased', style: 'color:var(--red-fg);background:var(--red-soft);' },
    reactivated: { label: 'Restarted', style: 'color:var(--peri);background:var(--peri-soft);' },
    dose_changed: { label: 'Dose changed', style: 'color:var(--amber-fg);background:var(--amber-soft);' },
  };
</script>

<div class="page-header">
  <div>
    <div class="page-title">Medication</div>
    <div class="page-subtitle">Current medications, today's doses &amp; history</div>
  </div>
  <div class="header-actions">
    <button class="add-med-btn" onclick={() => showAddMed = !showAddMed}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      Add medication
    </button>
    <button class="theme-btn" onclick={toggleTheme} aria-label="Toggle theme">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/></svg>
    </button>
  </div>
</div>

{#if banner}
  <div class="banner">
    <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="var(--amber)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"/></svg>
    <span>{bannerText}</span>
    <button class="banner-dismiss" onclick={() => banner = false}>Dismiss</button>
  </div>
{/if}

{#if showAddMed}
  <div class="add-form">
    <div class="card-heading">New medication</div>
    <div class="add-form-grid">
      <div class="text-field">
        <label for="nm-name">Name</label>
        <input id="nm-name" bind:value={nmName} placeholder="e.g. Sumatriptan" />
      </div>
      <div class="text-field">
        <label for="nm-dose">Dose</label>
        <input id="nm-dose" bind:value={nmDose} placeholder="50" class="center-input" />
      </div>
      <div class="text-field">
        <label for="nm-unit">Unit</label>
        <input id="nm-unit" bind:value={nmUnit} placeholder="mg" class="center-input" />
      </div>
      <div class="seg-field" role="radiogroup" aria-label="Medication type">
        <div class="seg-control">
          <button class="seg-btn" class:active={nmType === 'regular'} onclick={() => nmType = 'regular'}>Regular</button>
          <button class="seg-btn" class:active={nmType === 'prn'} onclick={() => nmType = 'prn'}>PRN</button>
        </div>
      </div>
      <button class="save-med-btn" onclick={addMedication}>Save</button>
    </div>
  </div>
{/if}

{#if loading}
  <p class="loading-text">Loading...</p>
{:else}
  <div class="med-layout">
    <div class="med-list-card">
      <div class="card-heading" style="padding:16px 18px 6px;">Current medications</div>
      <div class="section-divider" style="padding:6px 18px;">Regular</div>
      {#each regularMeds as med}
        <div class="med-row" class:dimmed={!med.active}>
          <span class="med-dot accent"></span>
          <div class="med-info">
            <div class="med-name">{med.name} <span class="med-freq">{med.short_code ?? ''}</span></div>
            <div class="med-detail">{med.category ?? ''}{med.default_dose != null ? ` · usual ${med.default_dose}${med.dose_unit || 'mg'}` : ''}</div>
          </div>
          {#if !med.active}
            <span class="ceased-badge">Ceased</span>
          {/if}
          <button class="toggle-active-btn" onclick={() => toggleActive(med)}>{med.active ? 'Cease' : 'Restart'}</button>
          {#if med.active}
            <button class="add-dose-btn" onclick={() => openDoseForm(med.id)}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
              Add dose
            </button>
          {/if}
        </div>
        {#if openId === med.id}
          <div class="dose-inline">
            <span class="dose-label">Amount</span>
            <input value={dosing[med.id]?.amount ?? ''} oninput={(e) => { const t = e.target as HTMLInputElement; if (!dosing[med.id]) dosing[med.id] = { amount: '', time: '' }; dosing[med.id].amount = t.value; }} class="dose-input" />
            <span class="dose-unit">{med.dose_unit || 'mg'}</span>
            <span class="dose-label" style="margin-left:6px;">at</span>
            <input value={dosing[med.id]?.time ?? ''} oninput={(e) => { const t = e.target as HTMLInputElement; if (!dosing[med.id]) dosing[med.id] = { amount: '', time: '' }; dosing[med.id].time = t.value; }} class="dose-input" />
            <button class="dose-save" onclick={() => saveDose(med.id)}>Log dose</button>
            <button class="dose-cancel" onclick={cancelDose}>Cancel</button>
          </div>
        {/if}
      {/each}

      <div class="section-divider">As needed (PRN)</div>
      {#each prnMeds as med}
        <div class="med-row" class:dimmed={!med.active}>
          <span class="med-dot peri"></span>
          <div class="med-info">
            <div class="med-name">{med.name}</div>
            <div class="med-detail">{med.category ?? ''}{med.default_dose != null ? ` · usual ${med.default_dose}${med.dose_unit || 'mg'}` : ''}</div>
          </div>
          {#if !med.active}
            <span class="ceased-badge">Ceased</span>
          {/if}
          <button class="toggle-active-btn" onclick={() => toggleActive(med)}>{med.active ? 'Cease' : 'Restart'}</button>
          {#if med.active}
            <button class="add-dose-btn" onclick={() => openDoseForm(med.id)}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
              Add dose
            </button>
          {/if}
        </div>
        {#if openId === med.id}
          <div class="dose-inline">
            <span class="dose-label">Amount</span>
            <input value={dosing[med.id]?.amount ?? ''} oninput={(e) => { const t = e.target as HTMLInputElement; if (!dosing[med.id]) dosing[med.id] = { amount: '', time: '' }; dosing[med.id].amount = t.value; }} class="dose-input" />
            <span class="dose-unit">{med.dose_unit || 'mg'}</span>
            <span class="dose-label" style="margin-left:6px;">at</span>
            <input value={dosing[med.id]?.time ?? ''} oninput={(e) => { const t = e.target as HTMLInputElement; if (!dosing[med.id]) dosing[med.id] = { amount: '', time: '' }; dosing[med.id].time = t.value; }} class="dose-input" />
            <button class="dose-save" onclick={() => saveDose(med.id)}>Log dose</button>
            <button class="dose-cancel" onclick={cancelDose}>Cancel</button>
          </div>
        {/if}
      {/each}
    </div>

    <div class="right-col">
      <div class="doses-card">
        <div class="doses-header">
          <span class="card-heading">Today's doses</span>
          <span class="doses-date">{formatDate(today)}</span>
        </div>
        {#if todayDoses.length === 0}
          <p class="empty-doses">No doses logged today</p>
        {:else}
          {#each todayDoses as dose}
            <div class="dose-row">
              <span class="dose-time">{dose.time_taken ?? '--:--'}</span>
              <div class="dose-med-name">{getMedName(dose.medication_id)} <span class="dose-amount">{dose.dose_amount != null ? `${dose.dose_amount} ${getMedUnit(dose.medication_id)}` : ''}</span></div>
              <button class="dose-delete" onclick={() => deleteDose(dose.medication_id, dose.time_taken)} aria-label="Delete dose">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
              </button>
            </div>
          {/each}
        {/if}
        <div class="doses-footer">{todayDoses.length} dose{todayDoses.length !== 1 ? 's' : ''} logged today</div>
      </div>
    </div>
  </div>

  <div class="history-card">
    <div class="card-heading" style="margin-bottom:8px;">Medication history</div>
    {#if history.length === 0}
      <p class="empty-text">No history recorded yet.</p>
    {:else}
      {#each history as h}
        <div class="hist-row">
          <div class="hist-badge-col">
            <span class="hist-badge" style={eventMeta[h.event_type]?.style ?? ''}>{eventMeta[h.event_type]?.label ?? h.event_type}</span>
            <span class="hist-date">{formatDate(h.event_date)}</span>
          </div>
          <div class="hist-info">
            <div class="hist-med">{h.medication_name}</div>
            <div class="hist-note">{h.detail ?? ''}</div>
          </div>
        </div>
      {/each}
    {/if}
  </div>
{/if}

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:22px; gap:16px; flex-wrap:wrap; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }
  .header-actions { display:flex; align-items:center; gap:10px; }
  .add-med-btn { display:inline-flex;align-items:center;gap:7px;background:var(--accent);color:#fff;border:none;border-radius:999px;padding:10px 16px;font-size:13px;font-weight:700;cursor:pointer; }
  .theme-btn { width:36px;height:36px;border-radius:50%;border:1px solid var(--border);background:var(--card);color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer; }

  .banner { display:flex;align-items:center;gap:11px;background:var(--amber-soft);border:1px solid var(--border);border-radius:14px;padding:12px 16px;margin-bottom:16px; }
  .banner span { font-size:13px;color:var(--amber-fg);font-weight:600;flex:1; }
  .banner-dismiss { border:none;background:transparent;color:var(--ts);cursor:pointer;font-size:13px;font-weight:700; }

  .add-form { background:var(--card);border:1px solid var(--border);border-radius:18px;padding:20px;box-shadow:var(--shadow);margin-bottom:16px;display:flex;flex-direction:column;gap:14px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:17px; font-weight:600; color:var(--tp); }
  .add-form-grid { display:flex; gap:12px; flex-wrap:wrap; align-items:end; }
  .text-field { display:flex; flex-direction:column; gap:6px; }
  .text-field label { font-size:12px; font-weight:700; color:var(--ts); }
  .text-field input { background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:10px 12px; font-size:13.5px; color:var(--tp); }
  .center-input { text-align:center; }
  .seg-field { display:flex; flex-direction:column; gap:6px; }
  .seg-control { display:flex; background:var(--inset); border:1px solid var(--border); border-radius:11px; padding:3px; gap:2px; }
  .seg-btn { flex:1; background:transparent; border:none; border-radius:9px; padding:8px; font-size:12.5px; font-weight:700; cursor:pointer; color:var(--ts); font-family:inherit; }
  .seg-btn.active { background:var(--accent); color:#fff; }
  .save-med-btn { background:var(--accent);color:#fff;border:none;border-radius:999px;padding:11px 18px;font-size:13px;font-weight:700;cursor:pointer; }

  .loading-text { color:var(--ts); text-align:center; padding:32px; }
  .empty-text { color:var(--ts); font-size:13px; padding:16px 0; }

  .med-layout { display:grid; grid-template-columns:1.6fr 1fr; gap:16px; align-items:start; }

  .med-list-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .section-divider { font-size:10.5px; letter-spacing:.07em; text-transform:uppercase; font-weight:800; color:var(--tm); border-top:1px solid var(--border); }

  .med-row { display:flex; align-items:center; gap:12px; padding:12px 18px; border-top:1px solid var(--border); }
  .med-row.dimmed { opacity:.5; }
  .med-dot { width:9px;height:9px;border-radius:50%;flex-shrink:0; }
  .med-dot.accent { background:var(--accent); }
  .med-dot.peri { background:var(--peri); }
  .med-info { flex:1; min-width:0; }
  .med-name { font-size:13.5px; font-weight:600; color:var(--tp); }
  .med-freq { font-size:11px; font-weight:600; color:var(--accent-fg); }
  .med-detail { font-size:11.5px; color:var(--tm); }
  .ceased-badge { font-size:10.5px; font-weight:700; color:var(--red-fg); background:var(--red-soft); padding:3px 9px; border-radius:999px; }
  .toggle-active-btn { border:none;background:transparent;color:var(--tm);font-size:12px;font-weight:600;cursor:pointer; }
  .add-dose-btn { display:inline-flex;align-items:center;gap:5px;background:var(--accent-soft);color:var(--accent-fg);border:1px solid var(--border);border-radius:999px;padding:7px 12px;font-size:12px;font-weight:700;cursor:pointer;white-space:nowrap;flex-shrink:0; }

  .dose-inline { display:flex;align-items:center;gap:9px;padding:12px 18px;background:var(--inset);border-top:1px solid var(--border); }
  .dose-label { font-size:11.5px; color:var(--ts); font-weight:600; }
  .dose-input { width:64px;background:var(--card);border:1px solid var(--border);border-radius:9px;padding:7px;font-size:12.5px;color:var(--tp);text-align:center;font-variant-numeric:tabular-nums; }
  .dose-unit { font-size:12px; color:var(--tm); }
  .dose-save { margin-left:auto;background:var(--accent);color:#fff;border:none;border-radius:999px;padding:8px 15px;font-size:12px;font-weight:700;cursor:pointer; }
  .dose-cancel { background:transparent;border:none;color:var(--ts);font-size:12px;font-weight:600;cursor:pointer; }

  .right-col { display:flex; flex-direction:column; gap:16px; }
  .doses-card { background:var(--card); border:1px solid var(--border); border-radius:18px; box-shadow:var(--shadow); overflow:hidden; }
  .doses-header { display:flex; justify-content:space-between; align-items:center; padding:16px 18px 12px; }
  .doses-date { font-size:11.5px; color:var(--tm); }
  .empty-doses { color:var(--ts); text-align:center; padding:24px; font-size:13px; }

  .dose-row { display:flex; align-items:center; gap:11px; padding:11px 18px; border-top:1px solid var(--border); }
  .dose-time { font-size:12px; color:var(--ts); font-variant-numeric:tabular-nums; width:42px; font-weight:600; }
  .dose-med-name { flex:1; min-width:0; font-size:13px; color:var(--tp); }
  .dose-amount { color:var(--tm); font-size:12px; }
  .dose-delete { width:24px;height:24px;border-radius:50%;border:none;background:transparent;color:var(--tm);display:flex;align-items:center;justify-content:center;cursor:pointer; }
  .doses-footer { padding:12px 18px; border-top:1px solid var(--border); font-size:12px; color:var(--tm); }

  .history-card { background:var(--card); border:1px solid var(--border); border-radius:18px; padding:22px; box-shadow:var(--shadow); margin-top:16px; display:flex; flex-direction:column; gap:4px; }
  .hist-row { display:flex; gap:14px; padding:12px 0; border-top:1px solid var(--border); }
  .hist-badge-col { width:96px; flex-shrink:0; display:flex; flex-direction:column; gap:5px; }
  .hist-badge { font-size:10.5px; font-weight:700; padding:3px 9px; border-radius:999px; text-align:center; }
  .hist-date { font-size:11.5px; color:var(--tm); font-variant-numeric:tabular-nums; text-align:center; }
  .hist-info { flex:1; min-width:0; display:flex; flex-direction:column; gap:5px; }
  .hist-med { font-size:13.5px; font-weight:600; color:var(--tp); }
  .hist-note { font-size:12.5px; color:var(--ts); }
</style>