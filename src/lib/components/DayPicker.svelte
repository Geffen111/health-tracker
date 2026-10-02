<script lang="ts">
  import { todayISO } from '$lib/formatDate';

  // The date between a page's day arrows, clickable to jump straight to a day. A hidden
  // native date input supplies the calendar; `max` stops at today, like the arrows do
  // (and `dateFromUrl`, which rejects future dates).
  let { date, label, onpick } = $props<{
    date: string;
    label: string;
    onpick: (date: string) => void;
  }>();

  let input: HTMLInputElement;

  function open() {
    input.value = date;
    try {
      input.showPicker();
    } catch {
      input.focus();
      input.click();
    }
  }

  function picked() {
    const d = input.value;
    if (d && d !== date && d <= todayISO()) onpick(d);
  }
</script>

<span class="day-picker">
  <button type="button" class="pick-btn" onclick={open} title="Pick a date">{label}</button>
  <input bind:this={input} type="date" max={todayISO()} onchange={picked} tabindex="-1" aria-hidden="true" />
</span>

<style>
  .day-picker { position: relative; display: inline-block; }
  .pick-btn {
    font: inherit; color: inherit; letter-spacing: inherit;
    background: transparent; border: none; border-radius: 8px;
    padding: 4px 6px; cursor: pointer;
  }
  .pick-btn:hover { background: var(--inset); }
  /* Rendered (showPicker needs that) but invisible; the calendar opens beneath the label. */
  input {
    position: absolute; left: 0; bottom: 0; width: 100%; height: 0;
    opacity: 0; pointer-events: none; border: 0; padding: 0;
  }
</style>
