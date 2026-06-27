<script lang="ts">
  import { confirmState, resolveConfirm } from '$lib/stores/confirm.svelte';

  function onKey(e: KeyboardEvent) {
    if (!confirmState.open) return;
    if (e.key === 'Escape') resolveConfirm(false);
    if (e.key === 'Enter') resolveConfirm(true);
  }
</script>

<svelte:window onkeydown={onKey} />

{#if confirmState.open}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay" role="presentation" onclick={() => resolveConfirm(false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_interactive_supports_focus -->
    <div class="dialog" role="alertdialog" aria-modal="true" aria-label={confirmState.title} tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="dialog-title">{confirmState.title}</div>
      <div class="dialog-message">{confirmState.message}</div>
      <div class="dialog-actions">
        <button class="btn-cancel" onclick={() => resolveConfirm(false)}>{confirmState.cancelLabel}</button>
        <button class="btn-confirm" class:danger={confirmState.danger} onclick={() => resolveConfirm(true)}>{confirmState.confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 2500;
    background: rgba(20, 29, 27, 0.42);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
    animation: overlay-in 0.14s ease-out;
  }
  .dialog {
    width: 100%;
    max-width: 380px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 22px 22px 18px;
    box-shadow: var(--shadow-lg);
    animation: dialog-in 0.16s ease-out;
  }
  .dialog-title {
    font-family: 'Source Serif 4', serif;
    font-size: 18px;
    font-weight: 600;
    color: var(--tp);
    margin-bottom: 8px;
  }
  .dialog-message {
    font-size: 13.5px;
    color: var(--ts);
    line-height: 1.5;
    margin-bottom: 20px;
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  .btn-cancel {
    background: var(--inset);
    border: 1px solid var(--border);
    color: var(--ts);
    border-radius: 999px;
    padding: 9px 18px;
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
  }
  .btn-cancel:hover { background: var(--border); }
  .btn-confirm {
    background: var(--accent);
    border: none;
    color: #fff;
    border-radius: 999px;
    padding: 9px 18px;
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
  }
  .btn-confirm.danger { background: var(--red); }
  .btn-confirm:hover { filter: brightness(0.96); }
  @keyframes overlay-in { from { opacity: 0; } to { opacity: 1; } }
  @keyframes dialog-in {
    from { opacity: 0; transform: translateY(8px) scale(0.98); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
</style>
