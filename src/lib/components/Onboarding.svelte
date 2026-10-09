<script lang="ts">
  import { features, setFeatures, type Features } from '$lib/stores/features.svelte';

  // First run, once the data folder is chosen: pick what to track. Everything here can be
  // changed later in Settings; the advanced extras aren't offered here at all.

  type Module = 'sleep' | 'activity' | 'cardio' | 'medication' | 'food' | 'work' | 'pacing';
  const MODULES: { key: Module; label: string; about: string }[] = [
    { key: 'sleep', label: 'Sleep', about: 'Hours, quality and how rested you felt' },
    { key: 'activity', label: 'Activity', about: 'What you did and how much it took out of you' },
    { key: 'pacing', label: 'Pacing', about: 'Activity load over time, beside your fatigue' },
    { key: 'cardio', label: 'Blood pressure & heart rate', about: 'Readings from a cuff or monitor' },
    { key: 'medication', label: 'Medication', about: 'Doses, schedules and changes' },
    { key: 'food', label: 'Food & drink', about: 'What you had each day, beside how you felt' },
    { key: 'work', label: 'Work', about: 'Hours worked' },
  ];

  let picks = $state<Record<Module, boolean>>(
    Object.fromEntries(MODULES.map((m) => [m.key, features[m.key]])) as Record<Module, boolean>,
  );
  let saving = $state(false);

  async function start() {
    saving = true;
    try {
      await setFeatures({ ...(picks as Partial<Features>), onboarded: true });
    } finally {
      saving = false;
    }
  }
</script>

<div class="wrap">
  <div class="card">
    <div class="title">What would you like to track?</div>
    <p>
      Every day starts with the <strong>Daily Log</strong>: fatigue, symptoms and a note. Pick anything
      else you want alongside it. You can change this at any time in Settings.
    </p>
    <div class="list">
      {#each MODULES as m (m.key)}
        <label class="item" class:on={picks[m.key]}>
          <input type="checkbox" bind:checked={picks[m.key]} />
          <span class="text">
            <span class="label">{m.label}</span>
            <span class="about">{m.about}</span>
          </span>
        </label>
      {/each}
    </div>
    <p class="small">
      Optional extras (importing watch data, reading a folder of health records, and AI features) are
      in <strong>Settings → Advanced</strong>. They're all off unless you turn them on.
    </p>
    <button class="primary" onclick={start} disabled={saving}>{saving ? 'Saving…' : 'Start tracking'}</button>
  </div>
</div>

<style>
  .wrap { display:flex; justify-content:center; padding:48px 16px; }
  .card { width:100%; max-width:600px; background:var(--card); border:1px solid var(--border); border-radius:20px; padding:28px; box-shadow:var(--shadow); display:flex; flex-direction:column; gap:14px; }
  .title { font-family:'Source Serif 4',serif; font-size:24px; font-weight:600; color:var(--tp); }
  p { font-size:13.5px; color:var(--ts); line-height:1.6; }
  p.small { font-size:12.5px; }
  p strong { color:var(--tp); }
  .list { display:grid; grid-template-columns:1fr 1fr; gap:8px; }
  @media (max-width: 640px) { .list { grid-template-columns:1fr; } }
  .item { display:flex; align-items:flex-start; gap:10px; padding:11px 13px; border:1px solid var(--border); border-radius:13px; cursor:pointer; }
  .item.on { border-color:var(--accent); background:var(--accent-soft); }
  .item input { accent-color:var(--accent); margin-top:2px; }
  .text { display:flex; flex-direction:column; gap:2px; }
  .label { font-size:13.5px; font-weight:700; color:var(--tp); }
  .about { font-size:12px; color:var(--ts); }
  .primary { align-self:flex-start; background:var(--accent); color:#fff; border:none; border-radius:999px; padding:11px 20px; font-size:13.5px; font-weight:700; cursor:pointer; }
  .primary:disabled { opacity:.6; }
</style>
