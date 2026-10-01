<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import WeeklyReport from '$lib/components/WeeklyReport.svelte';
  import { weekly, refreshWeeklyBanner, markWeeklySeen } from '$lib/stores/weekly.svelte';
  import { showToast } from '$lib/stores/toast.svelte';
  import { formatDate, todayISO, shiftISO } from '$lib/formatDate';

  let summaries = $state<any[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const latest = $derived(summaries[0] ?? null);
  const older = $derived(summaries.slice(1));

  /** Monday of the most recent complete week, as the backend counts it. */
  function lastFullWeekStart(): string {
    const today = new Date(todayISO() + 'T00:00:00');
    const sinceMonday = (today.getDay() + 6) % 7;
    return shiftISO(todayISO(), -(sinceMonday + 7));
  }

  async function load() {
    try {
      summaries = await invoke<any[]>('list_weekly_summaries');
      // Opening the page is what counts as reading it: clears the banner and sidebar dot.
      if (summaries[0] && !summaries[0].seen) {
        await markWeeklySeen(summaries[0].week_start);
        summaries[0].seen = true;
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function generate(weekStart: string) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      await invoke('generate_weekly_summary', { weekStart });
      await load();
      await refreshWeeklyBanner();
      showToast('Weekly summary ready');
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(load);

  // The launch hook may still be writing the summary when this page opens; pick it up.
  $effect(() => {
    if (!weekly.generating && weekly.banner && !loading && (!latest || latest.week_start < weekly.banner.week_start)) {
      load();
    }
  });
</script>

<div class="page-header">
  <div>
    <div class="page-title">Weekly summary</div>
    <div class="page-subtitle">
      {#if latest}Week of {formatDate(latest.week_start)} – {formatDate(latest.week_end)}{:else}A write-up of each week's log, made when you first open the app on a Monday{/if}
    </div>
  </div>
  <div class="header-actions">
    {#if latest}
      <button class="ghost-btn" onclick={() => generate(latest.week_start)} disabled={busy}>
        {busy ? 'Writing…' : 'Regenerate'}
      </button>
    {/if}
  </div>
</div>

{#if error}
  <p class="error">{error}</p>
{/if}

{#if loading}
  <p class="loading-text">Loading...</p>
{:else if latest}
  <WeeklyReport summary={latest} />

  {#if older.length}
    <details class="older">
      <summary>Previous summaries ({older.length})</summary>
      <div class="older-list">
        {#each older as s (s.week_start)}
          <details class="older-item">
            <summary>
              Week of {formatDate(s.week_start)} – {formatDate(s.week_end)}
              <span class="older-headline">{s.narrative.headline}</span>
            </summary>
            <div class="older-body">
              <WeeklyReport summary={s} />
              <button class="ghost-btn" onclick={() => generate(s.week_start)} disabled={busy}>Regenerate this week</button>
            </div>
          </details>
        {/each}
      </div>
    </details>
  {/if}
{:else}
  <div class="empty">
    {#if weekly.generating || busy}
      <p>Writing last week's summary… this takes about half a minute.</p>
    {:else}
      <p>No summary yet. One is written automatically the first time you open the app after a week ends, once an OpenRouter key is set in Settings.</p>
      <button class="primary-btn" onclick={() => generate(lastFullWeekStart())} disabled={busy}>Write last week's summary now</button>
    {/if}
  </div>
{/if}

<style>
  .page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
  .page-title { font-family: 'Source Serif 4', serif; font-size: 30px; font-weight: 600; color: var(--tp); letter-spacing: -0.01em; }
  .page-subtitle { font-size: 13.5px; color: var(--ts); margin-top: 3px; }
  .header-actions { display: flex; gap: 10px; }
  .primary-btn { background: var(--accent); color: #fff; border: none; border-radius: 999px; padding: 10px 18px; font-size: 13px; font-weight: 700; cursor: pointer; }
  .ghost-btn { background: var(--card); color: var(--tp); border: 1px solid var(--border); border-radius: 999px; padding: 8px 16px; font-size: 13px; font-weight: 600; cursor: pointer; }
  .ghost-btn:disabled, .primary-btn:disabled { opacity: 0.6; cursor: default; }
  .loading-text { color: var(--ts); font-size: 14px; text-align: center; padding: 48px; }
  .error { background: var(--red-soft); color: var(--red-fg); border-radius: 12px; padding: 10px 14px; font-size: 13px; margin-bottom: 16px; }
  .empty { background: var(--card); border: 1px solid var(--border); border-radius: 16px; padding: 36px; text-align: center; color: var(--ts); font-size: 14px; display: flex; flex-direction: column; gap: 16px; align-items: center; }
  .empty p { margin: 0; max-width: 520px; line-height: 1.6; }

  .older { margin-top: 26px; }
  .older > summary { cursor: pointer; font-size: 14px; font-weight: 700; color: var(--ts); padding: 8px 2px; }
  .older-list { display: flex; flex-direction: column; gap: 10px; margin-top: 10px; }
  .older-item { background: var(--inset); border: 1px solid var(--border); border-radius: 14px; padding: 12px 16px; }
  .older-item > summary { cursor: pointer; font-size: 13.5px; font-weight: 700; color: var(--tp); display: flex; flex-direction: column; gap: 2px; }
  .older-headline { font-weight: 500; color: var(--ts); font-size: 13px; }
  .older-body { margin-top: 14px; display: flex; flex-direction: column; gap: 12px; align-items: flex-start; }
  .older-body :global(.report) { width: 100%; }
</style>
